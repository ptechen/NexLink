use crate::state::{AppCommand, PeerInfo, ProxyStatus, SharedState};
use anyhow::Result;
use nexlink_lib::identity::NodeIdentity;
use nexlink_lib::network::behaviour::NexlinkBehaviourEvent;
use nexlink_lib::network::swarm::build_client_swarm;
use nexlink_lib::proxy::{self, ProxyCredentials, CREDENTIALS_PROTOCOL};
use libp2p::futures::{AsyncReadExt, StreamExt};
use libp2p::swarm::SwarmEvent;
use libp2p::{autonat, identify, relay, rendezvous, Multiaddr, PeerId};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

/// Request proxy credentials from relay via the credentials stream protocol
async fn request_credentials(
    control: &mut libp2p_stream::Control,
    relay_peer_id: PeerId,
) -> Result<ProxyCredentials> {
    let mut stream = control
        .open_stream(relay_peer_id, CREDENTIALS_PROTOCOL)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to open credentials stream: {e}"))?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;

    let creds: ProxyCredentials = serde_json::from_slice(&buf)?;
    Ok(creds)
}

async fn refresh_proxy_credentials(
    shared: &Arc<RwLock<SharedState>>,
    control: &mut libp2p_stream::Control,
    relay_peer_id: PeerId,
) -> Result<ProxyCredentials> {
    let creds = request_credentials(control, relay_peer_id).await?;
    {
        let mut state = shared.write().await;
        state.proxy_username = Some(creds.username.clone());
        state.proxy_password = Some(creds.password.clone());
    }
    Ok(creds)
}

fn send_command_result(done: oneshot::Sender<Result<(), String>>, result: Result<(), String>) {
    let _ = done.send(result);
}

pub async fn run_swarm_task(
    app: AppHandle,
    mut cmd_rx: mpsc::Receiver<AppCommand>,
    shared: Arc<RwLock<SharedState>>,
    data_dir: String,
) -> Result<()> {
    let identity_path = std::path::Path::new(&data_dir).join("identity.json");
    let identity = NodeIdentity::load_or_generate_with_recovery(&identity_path)?;
    let peer_id = identity.peer_id();

    let data_dir_str = data_dir;

    // Load persisted network config
    let data_path = std::path::Path::new(&data_dir_str);
    let net_config = nexlink_lib::network_id::load_network_config(data_path);
    let initial_namespace = net_config.namespace.clone();
    info!(
        path = %data_path.display(),
        relay_addr = ?net_config.relay_addr,
        namespace = %net_config.namespace,
        mode = %net_config.mode,
        "Loaded network config"
    );

    // Update shared state with peer ID and network config
    {
        let mut state = shared.write().await;
        state.peer_id = peer_id.to_string();
        state.data_dir = data_dir_str.clone();
        state.namespace = initial_namespace;
        state.network_mode = net_config.mode.clone();
        state.network_name = net_config.network_name.clone();
        if let Some(ref addr) = net_config.relay_addr {
            state.relay_addr = addr.clone();
        }
    }

    let mut swarm = build_client_swarm(&identity).await?;
    let stream_control = swarm.behaviour().stream.new_control();

    // Listen on random QUIC port
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;

    let mut discover_tick = interval(Duration::from_secs(30));
    let mut relay_peer_id: Option<PeerId> = None;
    let mut relay_addr: Option<Multiaddr> = None;

    // Auto-connect to persisted relay
    if let Some(ref addr_str) = net_config.relay_addr {
        if let Ok(maddr) = addr_str.parse::<Multiaddr>() {
            if let Some(libp2p::multiaddr::Protocol::P2p(pid)) = maddr.iter().last() {
                relay_peer_id = Some(pid);
                relay_addr = Some(maddr.clone());

                let circuit_listen: Multiaddr = format!("{}/p2p-circuit", maddr).parse().unwrap();
                let _ = swarm.listen_on(circuit_listen);
                let _ = swarm.dial(maddr);
                info!(%pid, "Auto-connecting to persisted relay");
            }
        }
    }
    // Load last provider preference
    let mut connected_provider: Option<PeerId> = net_config
        .last_provider
        .as_ref()
        .and_then(|s| s.parse::<PeerId>().ok());
    let mut registered = false;
    let mut proxy_handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();
    let traffic_counter = nexlink_lib::traffic::TrafficCounter::new();
    let mut node_selector = nexlink_lib::node_score::NodeSelector::new();
    let mut traffic_tick = interval(Duration::from_secs(1));
    let mut last_bytes_sent: u64 = 0;
    let mut last_bytes_received: u64 = 0;
    let mut proxy_guard = nexlink_lib::sys_proxy::ProxyGuard::new();
    let mut proxy_credentials: Option<ProxyCredentials> = None;

    info!(%peer_id, "Swarm task started");
    let _ = app.emit("swarm-ready", peer_id.to_string());

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        debug!(%address, "Listening on");
                        swarm.add_external_address(address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id: remote, .. } => {
                        debug!(%remote, "Connection established");
                        // Only track non-relay peers in node selector
                        if Some(remote) != relay_peer_id {
                            node_selector.set_connected(remote, true);
                        }

                        if Some(remote) == relay_peer_id && proxy_credentials.is_none() {
                            let mut ctl = stream_control.clone();
                            match refresh_proxy_credentials(&shared, &mut ctl, remote).await {
                                Ok(creds) => {
                                    info!(username = %creds.username, "Received proxy credentials from relay");
                                    proxy_credentials = Some(creds);
                                }
                                Err(e) => {
                                    warn!("Failed to request credentials from relay: {e}");
                                }
                            }
                        }

                        // Client mode: discover providers (don't register — clients shouldn't be discoverable)
                        if Some(remote) == relay_peer_id && !registered {
                            registered = true;
                            let namespace = shared.read().await.namespace.clone();
                            if let Ok(ns) = rendezvous::Namespace::new(namespace) {
                                swarm.behaviour_mut().rendezvous_client.discover(
                                    Some(ns),
                                    None,
                                    None,
                                    remote,
                                );
                            }
                            info!("Discovering providers");
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Identify(
                        identify::Event::Received { peer_id: remote, .. }
                    )) => {
                        debug!(%remote, "Identified peer");
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Identify(_)) => {}
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::RendezvousClient(event)) => {
                        match event {
                            rendezvous::client::Event::Registered { namespace, .. } => {
                                info!(?namespace, "Registered with rendezvous (unexpected in client mode)");
                            }
                            rendezvous::client::Event::Discovered { registrations, .. } => {
                                let mut state = shared.write().await;
                                for reg in registrations {
                                    let discovered_peer = reg.record.peer_id();
                                    if discovered_peer == peer_id {
                                        continue; // skip self
                                    }
                                    let addrs: Vec<String> = reg.record.addresses().iter()
                                        .map(|a| a.to_string())
                                        .collect();

                                    if !state.discovered_peers.iter().any(|p| p.peer_id == discovered_peer.to_string()) {
                                        let peer_info = PeerInfo {
                                            peer_id: discovered_peer.to_string(),
                                            addrs,
                                            is_provider: true,
                                            latency_ms: None,
                                            connected: connected_provider == Some(discovered_peer),
                                        };
                                        state.discovered_peers.push(peer_info.clone());
                                        let _ = app.emit("peer-discovered", &peer_info);
                                    }

                                    // Dial the discovered peer if not already connected
                                    if !swarm.is_connected(&discovered_peer) {
                                        if let Err(e) = swarm.dial(discovered_peer) {
                                            warn!(%discovered_peer, "Failed to dial: {e}");
                                        }
                                    }
                                }
                            }
                            other => {
                                debug!("Rendezvous: {other:?}");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Autonat(
                        autonat::Event::StatusChanged { new, .. }
                    )) => {
                        let nat_str = match &new {
                            autonat::NatStatus::Public(_) => "Public",
                            autonat::NatStatus::Private => "Private",
                            autonat::NatStatus::Unknown => "Unknown",
                        };
                        {
                            let mut state = shared.write().await;
                            state.nat_status = nat_str.to_string();
                        }
                        let _ = app.emit("nat-status", nat_str);
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Autonat(_)) => {}
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::RelayClient(event)) => {
                        match event {
                            relay::client::Event::ReservationReqAccepted { relay_peer_id: peer, renewal, .. } => {
                                info!(%peer, %renewal, "Relay reservation accepted");
                            }
                            relay::client::Event::InboundCircuitEstablished { src_peer_id, .. } => {
                                info!(%src_peer_id, "Inbound circuit through relay");
                            }
                            relay::client::Event::OutboundCircuitEstablished { relay_peer_id: peer, .. } => {
                                info!(%peer, "Outbound circuit through relay");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Ping(event)) => {
                        let remote = event.peer;
                        match event.result {
                            Ok(rtt) => {
                                let rtt_ms = rtt.as_millis() as u64;
                                // Only track non-relay peers for provider selection
                                if Some(remote) != relay_peer_id {
                                    node_selector.update_latency(remote, rtt_ms);
                                    node_selector.record_success(remote);
                                }

                                // Update latency in discovered_peers and is_selected
                                let _best = node_selector.current();
                                {
                                    let mut state = shared.write().await;
                                    for p in state.discovered_peers.iter_mut() {
                                        if p.peer_id == remote.to_string() {
                                            p.latency_ms = Some(rtt_ms);
                                        }
                                    }
                                }

                                // Auto-select best node if no provider connected yet
                                if connected_provider.is_none() {
                                    if let Some(new_best) = node_selector.select_best() {
                                        info!(%new_best, "Auto-selected best provider");
                                        connected_provider = Some(new_best);
                                        let mut state = shared.write().await;
                                        state.connected_peer = Some(new_best.to_string());
                                        for p in state.discovered_peers.iter_mut() {
                                            p.connected = p.peer_id == new_best.to_string();
                                        }
                                        let _ = app.emit("peer-connected", new_best.to_string());

                                        // Persist last provider
                                        let data_path = std::path::Path::new(&data_dir_str);
                                        let mut net_cfg = nexlink_lib::network_id::load_network_config(data_path);
                                        net_cfg.last_provider = Some(new_best.to_string());
                                        let _ = nexlink_lib::network_id::save_network_config(data_path, &net_cfg);
                                    }
                                }
                            }
                            Err(_) => {
                                if Some(remote) != relay_peer_id {
                                    node_selector.record_failure(remote);
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Stream(_)) => {}
                    SwarmEvent::ConnectionClosed { peer_id: remote, .. } => {
                        debug!(%remote, "Disconnected");
                        if Some(remote) != relay_peer_id {
                            node_selector.set_connected(remote, false);
                        }
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id: Some(remote), error, .. } => {
                        let err_str = error.to_string();
                        warn!(%remote, "Connection failed: {error}");
                        // Retry via relay circuit, but not if already a circuit failure (avoid loops)
                        if let (Some(ref r_addr), Some(r_peer)) = (&relay_addr, relay_peer_id) {
                            if remote != r_peer && !err_str.contains("p2p-circuit") {
                                if let Ok(circuit_addr) = format!(
                                    "{}/p2p-circuit/p2p/{}", r_addr, remote
                                ).parse::<Multiaddr>() {
                                    let _ = swarm.dial(
                                        libp2p::swarm::dial_opts::DialOpts::peer_id(remote)
                                            .addresses(vec![circuit_addr])
                                            .condition(libp2p::swarm::dial_opts::PeerCondition::Always)
                                            .build()
                                    );
                                }
                            }
                        }
                    }
                    SwarmEvent::ListenerError { listener_id, error } => {
                        warn!(?listener_id, "Listener error: {error}");
                    }
                    _ => {}
                }
            }

            _ = traffic_tick.tick() => {
                let snap = traffic_counter.snapshot();
                let upload_speed = snap.bytes_sent.saturating_sub(last_bytes_sent);
                let download_speed = snap.bytes_received.saturating_sub(last_bytes_received);
                last_bytes_sent = snap.bytes_sent;
                last_bytes_received = snap.bytes_received;

                let mut state = shared.write().await;
                state.traffic.bytes_sent = snap.bytes_sent;
                state.traffic.bytes_received = snap.bytes_received;
                state.traffic.upload_speed = upload_speed;
                state.traffic.download_speed = download_speed;
                state.traffic.active_connections = snap.active_connections;
            }

            _ = discover_tick.tick() => {
                if let Some(relay) = relay_peer_id {
                    if proxy_credentials.is_none() {
                        let mut ctl = stream_control.clone();
                        match refresh_proxy_credentials(&shared, &mut ctl, relay).await {
                            Ok(creds) => {
                                info!(username = %creds.username, "Refreshed proxy credentials from relay");
                                proxy_credentials = Some(creds);
                            }
                            Err(e) => {
                                warn!("Failed to refresh credentials from relay: {e}");
                            }
                        }
                    }

                    if registered {
                        let namespace = shared.read().await.namespace.clone();
                        if let Ok(ns) = rendezvous::Namespace::new(namespace) {
                            swarm.behaviour_mut().rendezvous_client.discover(
                                Some(ns),
                                None,
                                None,
                                relay,
                            );
                        }
                    }
                }
            }

            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    AppCommand::StartProxy { unified_port, done } => {
                        let result = if let Some(provider_peer) = connected_provider {
                            if let Some(creds) = proxy_credentials.clone() {
                                let control = stream_control.clone();
                                let tc = traffic_counter.clone();

                                let h = tokio::spawn(async move {
                                    if let Err(e) = proxy::unified_proxy::start_unified_proxy(unified_port, provider_peer, control, tc, creds).await {
                                        warn!("Unified proxy error: {e}");
                                    }
                                });
                                proxy_handles.push(h);

                                let status = ProxyStatus { running: true, unified_port };
                                shared.write().await.proxy_status = Some(status.clone());
                                let _ = app.emit("proxy-status", &status);
                                info!(unified_port, %provider_peer, "Unified proxy started");
                                Ok(())
                            } else {
                                let msg = "No proxy credentials available, cannot start proxy".to_string();
                                warn!("{msg}");
                                Err(msg)
                            }
                        } else {
                            let msg = "No provider connected, cannot start proxy".to_string();
                            warn!("{msg}");
                            Err(msg)
                        };
                        send_command_result(done, result);
                    }
                    AppCommand::StopProxy { done } => {
                        // Auto-clear system proxy if active
                        if proxy_guard.is_active() {
                            let _ = nexlink_lib::sys_proxy::clear_system_proxy();
                            proxy_guard.deactivate();
                            shared.write().await.system_proxy_enabled = false;
                            let _ = app.emit("system-proxy-changed", false);
                        }
                        for h in proxy_handles.drain(..) {
                            h.abort();
                        }
                        let status = ProxyStatus { running: false, unified_port: 7890 }; // Default unified port
                        shared.write().await.proxy_status = Some(status.clone());
                        let _ = app.emit("proxy-status", &status);
                        info!("Proxy stopped");
                        send_command_result(done, Ok(()));
                    }
                    AppCommand::ConnectNode { peer_id: target } => {
                        if let Ok(pid) = target.parse::<PeerId>() {
                            let _ = swarm.dial(pid);
                            connected_provider = Some(pid);
                            node_selector.set_current(Some(pid));
                            {
                                let mut state = shared.write().await;
                                state.connected_peer = Some(target.clone());
                                for peer in state.discovered_peers.iter_mut() {
                                    peer.connected = peer.peer_id == target;
                                }
                            }
                            let _ = app.emit("peer-connected", &target);

                            // Persist last provider
                            let data_path = std::path::Path::new(&data_dir_str);
                            let mut net_cfg = nexlink_lib::network_id::load_network_config(data_path);
                            net_cfg.last_provider = Some(target.clone());
                            let _ = nexlink_lib::network_id::save_network_config(data_path, &net_cfg);

                            info!(%target, "Connecting to provider");
                        }
                    }
                    AppCommand::DisconnectNode => {
                        if let Some(provider_peer) = connected_provider.take() {
                            let _ = swarm.disconnect_peer_id(provider_peer);
                            // Also stop proxy
                            for h in proxy_handles.drain(..) {
                                h.abort();
                            }
                            let mut state = shared.write().await;
                            state.connected_peer = None;
                            state.proxy_status = Some(ProxyStatus { running: false, unified_port: 7890 }); // Default unified port
                        }
                        let _ = app.emit("peer-disconnected", ());
                    }
                    AppCommand::RefreshNodes => {
                        if let Some(relay) = relay_peer_id {
                            let namespace = shared.read().await.namespace.clone();
                            if let Ok(ns) = rendezvous::Namespace::new(namespace) {
                                swarm.behaviour_mut().rendezvous_client.discover(
                                    Some(ns),
                                    None,
                                    None,
                                    relay,
                                );
                            }
                        }
                    }
                    AppCommand::UpdateConfig { relay_addr: new_relay, namespace: new_ns } => {
                        let new_relay = new_relay.and_then(|addr| {
                            let trimmed = addr.trim().to_string();
                            if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed)
                            }
                        });

                        if let Some(addr_str) = new_relay.clone() {
                            // Parse and dial new relay
                            if let Ok(maddr) = addr_str.parse::<Multiaddr>() {
                                if let Some(libp2p::multiaddr::Protocol::P2p(pid)) = maddr.iter().last() {
                                    relay_peer_id = Some(pid);
                                    relay_addr = Some(maddr.clone());
                                    registered = false;

                                    let circuit_listen: Multiaddr = format!("{}/p2p-circuit", maddr).parse().unwrap();
                                    let _ = swarm.listen_on(circuit_listen);
                                    let _ = swarm.dial(maddr);
                                    info!(%pid, "Connecting to new relay");
                                }
                            }
                        } else {
                            relay_peer_id = None;
                            relay_addr = None;
                            registered = false;
                            proxy_credentials = None;
                        }

                        {
                            let mut state = shared.write().await;
                            if new_relay.is_some() {
                                state.relay_addr = new_relay.clone().unwrap_or_default();
                            }
                            if let Some(ns) = new_ns {
                                state.namespace = ns;
                            }
                        }
                    }
                    AppCommand::JoinNetwork { name, password } => {
                        let mut config = nexlink_lib::network_id::NetworkConfig::private(&name, &password);
                        let new_namespace = config.namespace.clone();

                        // Stop proxy and disconnect if active
                        for h in proxy_handles.drain(..) {
                            h.abort();
                        }
                        if let Some(provider_peer) = connected_provider.take() {
                            let _ = swarm.disconnect_peer_id(provider_peer);
                        }

                        // Preserve existing relay_addr when saving
                        let data_path = std::path::Path::new(&data_dir_str);
                        let existing = nexlink_lib::network_id::load_network_config(data_path);
                        config.relay_addr = existing.relay_addr;
                        if let Err(e) = nexlink_lib::network_id::save_network_config(data_path, &config) {
                            warn!("Failed to save network config: {e}");
                        }

                        // Update shared state
                        {
                            let mut state = shared.write().await;
                            state.namespace = new_namespace.clone();
                            state.network_mode = "private".to_string();
                            state.network_name = Some(name.clone());
                            state.connected_peer = None;
                            state.discovered_peers.clear();
                            state.proxy_status = Some(ProxyStatus { running: false, unified_port: 7890 });
                        }

                        // Discover providers in new namespace
                        registered = true;
                        if let Some(relay) = relay_peer_id {
                            if let Ok(ns) = rendezvous::Namespace::new(new_namespace) {
                                swarm.behaviour_mut().rendezvous_client.discover(
                                    Some(ns), None, None, relay,
                                );
                            }
                        }

                        let _ = app.emit("network-changed", "private");
                        info!(%name, "Joined private network");
                    }
                    AppCommand::LeaveNetwork => {
                        let mut config = nexlink_lib::network_id::NetworkConfig::public();
                        let new_namespace = config.namespace.clone();

                        // Stop proxy and disconnect
                        for h in proxy_handles.drain(..) {
                            h.abort();
                        }
                        if let Some(provider_peer) = connected_provider.take() {
                            let _ = swarm.disconnect_peer_id(provider_peer);
                        }

                        // Save public config, preserving relay_addr
                        let data_path = std::path::Path::new(&data_dir_str);
                        let existing = nexlink_lib::network_id::load_network_config(data_path);
                        config.relay_addr = existing.relay_addr;
                        if let Err(e) = nexlink_lib::network_id::save_network_config(data_path, &config) {
                            warn!("Failed to save network config: {e}");
                        }

                        // Update shared state
                        {
                            let mut state = shared.write().await;
                            state.namespace = new_namespace.clone();
                            state.network_mode = "public".to_string();
                            state.network_name = None;
                            state.connected_peer = None;
                            state.discovered_peers.clear();
                            state.proxy_status = Some(ProxyStatus { running: false, unified_port: 7890 });
                        }

                        // Discover providers in public namespace
                        registered = true;
                        if let Some(relay) = relay_peer_id {
                            if let Ok(ns) = rendezvous::Namespace::new(new_namespace) {
                                swarm.behaviour_mut().rendezvous_client.discover(
                                    Some(ns), None, None, relay,
                                );
                            }
                        }

                        let _ = app.emit("network-changed", "public");
                        info!("Left private network, back to public");
                    }
                    AppCommand::SetSystemProxy { done } => {
                        let state = shared.read().await;
                        let result = if let Some(ref ps) = state.proxy_status {
                            if ps.running {
                                let unified_port = ps.unified_port;
                                drop(state);
                                match nexlink_lib::sys_proxy::set_system_proxy(unified_port, unified_port) {  // Use unified port for both
                                    Ok(()) => {
                                        proxy_guard.activate();
                                        shared.write().await.system_proxy_enabled = true;
                                        let _ = app.emit("system-proxy-changed", true);
                                        info!("System proxy enabled");
                                        Ok(())
                                    }
                                    Err(e) => {
                                        let msg = format!("Failed to set system proxy: {e}");
                                        warn!("{msg}");
                                        Err(msg)
                                    }
                                }
                            } else {
                                let msg = "Cannot set system proxy: proxy not running".to_string();
                                warn!("{msg}");
                                Err(msg)
                            }
                        } else {
                            let msg = "Cannot set system proxy: proxy not initialized".to_string();
                            warn!("{msg}");
                            Err(msg)
                        };
                        send_command_result(done, result);
                    }
                    AppCommand::ClearSystemProxy { done } => {
                        let result = if let Err(e) = nexlink_lib::sys_proxy::clear_system_proxy() {
                            let msg = format!("Failed to clear system proxy: {e}");
                            warn!("{msg}");
                            Err(msg)
                        } else {
                            proxy_guard.deactivate();
                            shared.write().await.system_proxy_enabled = false;
                            let _ = app.emit("system-proxy-changed", false);
                            info!("System proxy disabled");
                            Ok(())
                        };
                        if result.is_err() {
                            proxy_guard.deactivate();
                            shared.write().await.system_proxy_enabled = false;
                            let _ = app.emit("system-proxy-changed", false);
                        }
                        send_command_result(done, result);
                    }
                }
            }
        }
    }
}
