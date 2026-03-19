use anyhow::Result;
use clap::Parser;
use dashmap::DashMap;
use libp2p::futures::{AsyncReadExt, StreamExt};
use libp2p::rendezvous;
use libp2p::swarm::SwarmEvent;
use libp2p::Multiaddr;
use libp2p::{autonat, ping, relay};
use nexlink_lib::config::default_data_dir;
use nexlink_lib::identity::NodeIdentity;
use nexlink_lib::network::behaviour::NexlinkBehaviourEvent;
use nexlink_lib::network::swarm::build_client_swarm;
use nexlink_lib::proxy::{ProxyCredentials, CREDENTIALS_PROTOCOL, CREDENTIALS_SYNC_PROTOCOL};
use nexlink_taos::{TaosClient, TaosConfig, TrafficWriteRepository};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::{signal, time};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "nexlink-node", about = "NexLink P2P proxy node")]
struct Cli {
    /// Relay server multiaddr (e.g. /ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>)
    #[arg(short, long)]
    relay: String,

    /// Namespace for rendezvous
    #[arg(short, long, default_value = "nexlink-public")]
    namespace: String,

    /// Run as provider (proxy service provider)
    #[arg(short = 'p', long, default_value_t = false)]
    provider: bool,

    /// Unified proxy port supporting both SOCKS5 and HTTP CONNECT (client mode)
    #[arg(long, default_value_t = 7890)]
    unified_port: u16,

    /// Data directory (default: ~/.nexlink/node)
    #[arg(short, long)]
    data_dir: Option<String>,
}

/// Request proxy credentials from relay via the credentials stream protocol
async fn request_credentials(
    control: &mut libp2p_stream::Control,
    relay_peer_id: libp2p::PeerId,
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

/// Request all client credentials from relay via the credentials sync protocol (for provider verification)
async fn request_credentials_sync(
    control: &mut libp2p_stream::Control,
    relay_peer_id: libp2p::PeerId,
) -> Result<Vec<ProxyCredentials>> {
    let mut stream = control
        .open_stream(relay_peer_id, CREDENTIALS_SYNC_PROTOCOL)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to open credentials sync stream: {e}"))?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;

    let creds: Vec<ProxyCredentials> = serde_json::from_slice(&buf)?;
    Ok(creds)
}

async fn sync_allowed_credentials(
    allowed_credentials: &Arc<DashMap<String, String>>,
    control: &mut libp2p_stream::Control,
    relay_peer_id: libp2p::PeerId,
) -> Result<usize> {
    let creds_list = request_credentials_sync(control, relay_peer_id).await?;
    allowed_credentials.clear();
    for c in &creds_list {
        allowed_credentials.insert(c.username.clone(), c.password.clone());
    }
    Ok(creds_list.len())
}

fn is_ping_timeout(failure: &ping::Failure) -> bool {
    match failure {
        ping::Failure::Timeout => true,
        ping::Failure::Unsupported => false,
        ping::Failure::Other { error } => error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_error| io_error.kind() == std::io::ErrorKind::TimedOut),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let data_dir = cli
        .data_dir
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| default_data_dir().join("node"));
    let identity_path = data_dir.join("identity.json");
    let identity = NodeIdentity::load_or_generate_with_recovery(&identity_path)?;

    let rules_path = data_dir.join("proxy_rules.json");
    if let Err(e) = nexlink_lib::pac::load_rules(&rules_path) {
        warn!(path = %rules_path.display(), "Failed to load proxy rules: {e:#}");
    } else {
        info!(path = %rules_path.display(), count = nexlink_lib::pac::rule_count(), "Loaded proxy rules");
    }

    info!(peer_id = %identity.peer_id(), "Starting nexlink node");

    let mut swarm = build_client_swarm(&identity).await?;

    // Get a stream::Control handle for proxy operations
    let stream_control = swarm.behaviour().stream.new_control();

    // If running as provider, spawn the incoming proxy stream handler with credential verification
    let allowed_credentials: Arc<DashMap<String, String>> = Arc::new(DashMap::new());
    if cli.provider {
        let mut accept_control = stream_control.clone();
        let mut incoming = accept_control
            .accept(nexlink_lib::proxy::PROXY_PROTOCOL)
            .expect("proxy protocol not yet registered");

        let creds_map = allowed_credentials.clone();
        tokio::spawn(async move {
            use libp2p::futures::StreamExt;
            while let Some((peer_id, stream)) = incoming.next().await {
                let map = creds_map.clone();
                tokio::spawn(async move {
                    if let Err(e) = nexlink_lib::proxy::provider_handler::handle_proxy_stream(
                        peer_id,
                        stream,
                        None,
                        Some(&map),
                    )
                    .await
                    {
                        warn!(%peer_id, "Proxy stream error: {e:#}");
                    }
                });
            }
        });
        info!("Provider: accepting proxy streams");
    }

    // Listen on random QUIC port
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;

    // Parse relay address and extract peer ID
    let relay_addr: Multiaddr = cli.relay.parse().expect("Invalid relay address");
    let relay_peer_id = relay_addr
        .iter()
        .find_map(|p| match p {
            libp2p::multiaddr::Protocol::P2p(peer_id) => Some(peer_id),
            _ => None,
        })
        .expect("Relay address must contain /p2p/<peer_id>");

    // Dial the relay server
    swarm.dial(relay_addr.clone())?;
    info!(%relay_peer_id, "Dialing relay server");

    // Listen via relay circuit so other peers can reach us through the relay
    let relay_circuit_addr: Multiaddr = format!("{relay_addr}/p2p-circuit").parse()?;
    swarm.listen_on(relay_circuit_addr)?;

    let namespace = rendezvous::Namespace::new(cli.namespace.clone()).expect("Invalid namespace");
    let mut registered = false;
    let mut proxy_started = false;
    let mut discover_interval = time::interval(Duration::from_secs(30));
    let mut relay_retry_interval = time::interval(Duration::from_secs(5));
    let mut traffic_flush_interval = time::interval(Duration::from_secs(10));
    let mut relay_dial_in_flight = true;
    let mut proxy_credentials: Option<ProxyCredentials> = None;
    let mut last_flushed_totals: HashMap<libp2p::PeerId, (u64, u64)> = HashMap::new();
    let taos_repo = TrafficWriteRepository::new(TaosClient::new(TaosConfig::from_env()));

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!(%address, "Listening");
                        swarm.add_external_address(address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        info!(%peer_id, "Connected to peer");

                        if peer_id == relay_peer_id {
                            relay_dial_in_flight = false;
                            if !cli.provider && proxy_credentials.is_none() {
                                let mut ctl = stream_control.clone();
                                match request_credentials(&mut ctl, relay_peer_id).await {
                                    Ok(creds) => {
                                        info!(username = %creds.username, "Received proxy credentials from relay");
                                        proxy_credentials = Some(creds);
                                    }
                                    Err(e) => {
                                        warn!("Failed to request credentials from relay: {e}");
                                    }
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::RendezvousClient(event)) => {
                        match event {
                            rendezvous::client::Event::Registered { namespace, .. } => {
                                info!(?namespace, "Registered with rendezvous");
                                registered = true;

                                if cli.provider {
                                    let mut ctl = stream_control.clone();
                                    match sync_allowed_credentials(&allowed_credentials, &mut ctl, relay_peer_id).await {
                                        Ok(count) => {
                                            info!(count, "Synced client credentials from relay");
                                        }
                                        Err(e) => {
                                            warn!("Failed to sync credentials from relay: {e}");
                                        }
                                    }
                                }

                                swarm.behaviour_mut().rendezvous_client.discover(
                                    Some(namespace),
                                    None,
                                    None,
                                    relay_peer_id,
                                );
                            }
                            rendezvous::client::Event::Discovered { registrations, .. } => {
                                for reg in registrations {
                                    let peer = reg.record.peer_id();
                                    if peer != *swarm.local_peer_id() {
                                        info!(%peer, "Discovered peer");
                                        if !swarm.is_connected(&peer) {
                                            if let Err(e) = swarm.dial(peer) {
                                                warn!(%peer, "Failed to dial peer: {e}");
                                            }
                                        }

                                        // In client mode, start local unified proxy pointing to the first discovered provider
                                        if !cli.provider && !proxy_started {
                                            if let Some(creds) = proxy_credentials.clone() {
                                                proxy_started = true;
                                                let unified_ctl = stream_control.clone();
                                                let unified_port = cli.unified_port;
                                                let unified_traffic = nexlink_lib::traffic::TrafficCounter::new();
                                                tokio::spawn(async move {
                                                    if let Err(e) = nexlink_lib::proxy::unified_proxy::start_unified_proxy(unified_port, peer, unified_ctl, unified_traffic, creds).await {
                                                        warn!("Unified proxy error: {e:#}");
                                                    }
                                                });
                                                info!(unified_port, %peer, "Started unified local proxy -> provider");
                                            } else {
                                                warn!("No proxy credentials available, cannot start proxy");
                                            }
                                        }
                                    }
                                }
                            }
                            other => {
                                info!("Rendezvous: {other:?}");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Ping(event)) => {
                        let peer_id = event.peer;
                        let connection = event.connection;
                        match event.result {
                            Ok(rtt) => {
                                debug!(%peer_id, ?connection, rtt_ms = rtt.as_millis(), "Ping ok");
                            }
                            Err(error) => {
                                warn!(%peer_id, ?connection, "Ping failed: {error}");
                                if peer_id == relay_peer_id && is_ping_timeout(&error) {
                                    registered = false;
                                    proxy_credentials = None;
                                    relay_dial_in_flight = false;
                                    if swarm.close_connection(connection) {
                                        info!(%peer_id, ?connection, "Closing timed-out relay connection");
                                    }
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Identify(
                        libp2p::identify::Event::Received {
                            peer_id,
                            info,
                            ..
                        },
                    )) => {
                        info!(%peer_id, observed_addr = %info.observed_addr, "Identified by peer");
                        if peer_id == relay_peer_id && !registered {
                            if cli.provider {
                                match swarm.behaviour_mut().rendezvous_client.register(
                                    namespace.clone(),
                                    relay_peer_id,
                                    None,
                                ) {
                                    Ok(()) => info!("Registering with rendezvous"),
                                    Err(e) => warn!("Failed to register: {e}"),
                                }
                            } else {
                                registered = true;
                                swarm.behaviour_mut().rendezvous_client.discover(
                                    Some(namespace.clone()),
                                    None,
                                    None,
                                    relay_peer_id,
                                );
                                info!("Client mode: discovering providers");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Identify(_)) => {}
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::Autonat(event)) => {
                        match event {
                            autonat::Event::StatusChanged { old, new } => {
                                info!(?old, ?new, "NAT status changed");
                                if let autonat::NatStatus::Public(addr) = &new {
                                    info!(%addr, "Publicly reachable");
                                }
                            }
                            _ => {
                                tracing::debug!("AutoNAT: {event:?}");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::RelayClient(event)) => {
                        match event {
                            relay::client::Event::ReservationReqAccepted {
                                relay_peer_id: peer,
                                renewal,
                                ..
                            } => {
                                info!(%peer, %renewal, "Relay reservation accepted");
                            }
                            relay::client::Event::InboundCircuitEstablished {
                                src_peer_id,
                                ..
                            } => {
                                info!(%src_peer_id, "Inbound circuit through relay");
                            }
                            relay::client::Event::OutboundCircuitEstablished {
                                relay_peer_id: peer,
                                ..
                            } => {
                                info!(%peer, "Outbound circuit through relay");
                            }
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        info!(%peer_id, "Disconnected");
                        if peer_id == relay_peer_id {
                            registered = false;
                            proxy_credentials = None;
                            relay_dial_in_flight = false;
                        }
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        if let Some(peer) = peer_id {
                            let err_str = error.to_string();
                            warn!(%peer, "Outgoing connection failed: {error}");
                            if peer == relay_peer_id {
                                registered = false;
                                proxy_credentials = None;
                                relay_dial_in_flight = false;
                            }
                            if peer != relay_peer_id && !err_str.contains("p2p-circuit") {
                                let circuit_addr: Multiaddr = format!(
                                    "{relay_addr}/p2p-circuit/p2p/{peer}"
                                ).parse().expect("valid circuit addr");
                                info!(%peer, "Retrying via relay circuit");
                                if let Err(e) = swarm.dial(
                                    libp2p::swarm::dial_opts::DialOpts::peer_id(peer)
                                        .addresses(vec![circuit_addr])
                                        .condition(libp2p::swarm::dial_opts::PeerCondition::Always)
                                        .build()
                                ) {
                                    warn!(%peer, "Relay circuit dial failed: {e}");
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
            _ = discover_interval.tick() => {
                if registered {
                    info!("Periodic peer discovery");
                    swarm.behaviour_mut().rendezvous_client.discover(
                        Some(namespace.clone()),
                        None,
                        None,
                        relay_peer_id,
                    );

                    // Provider: refresh credentials from relay on each discover tick
                    if cli.provider {
                        let mut ctl = stream_control.clone();
                        match sync_allowed_credentials(&allowed_credentials, &mut ctl, relay_peer_id).await {
                            Ok(count) => {
                                info!(count, "Refreshed client credentials from relay");
                            }
                            Err(e) => {
                                warn!("Failed to refresh credentials from relay: {e}");
                            }
                        }
                    } else if proxy_credentials.is_none() {
                        let mut ctl = stream_control.clone();
                        match request_credentials(&mut ctl, relay_peer_id).await {
                            Ok(creds) => {
                                info!(username = %creds.username, "Refreshed proxy credentials from relay");
                                proxy_credentials = Some(creds);
                            }
                            Err(e) => {
                                warn!("Failed to refresh credentials from relay: {e}");
                            }
                        }
                    }
                }
            }
            _ = relay_retry_interval.tick() => {
                if !swarm.is_connected(&relay_peer_id) && !relay_dial_in_flight {
                    match swarm.dial(relay_addr.clone()) {
                        Ok(()) => {
                            relay_dial_in_flight = true;
                            info!(%relay_peer_id, addr = %relay_addr, "Retrying relay connection");
                        }
                        Err(e) => {
                            warn!(%relay_peer_id, addr = %relay_addr, "Failed to redial relay: {e}");
                        }
                    }
                }
            }
            _ = traffic_flush_interval.tick() => {
                let snapshots = nexlink_traffic::snapshot_all();
                let snapshot_count = snapshots.len();
                if snapshot_count == 0 {
                    debug!("Traffic flush tick: no snapshots collected");
                    continue;
                }

                let delta_snapshots: Vec<_> = snapshots
                    .into_iter()
                    .filter_map(|mut snapshot| {
                        let previous = last_flushed_totals
                            .get(&snapshot.peer_id)
                            .copied()
                            .unwrap_or((0, 0));

                        let delta_upload = snapshot.upload.saturating_sub(previous.0);
                        let delta_download = snapshot.download.saturating_sub(previous.1);

                        last_flushed_totals
                            .insert(snapshot.peer_id, (snapshot.upload, snapshot.download));

                        if delta_upload == 0 && delta_download == 0 && snapshot.active_connections == 0 {
                            return None;
                        }

                        debug!(
                            peer_id = %snapshot.peer_id,
                            total_upload = snapshot.upload,
                            total_download = snapshot.download,
                            delta_upload,
                            delta_download,
                            active_connections = snapshot.active_connections,
                            role = ?snapshot.role,
                            source = ?snapshot.source,
                            source_ip = ?snapshot.source_ip,
                            source_transport = ?snapshot.source_transport,
                            "Prepared traffic delta snapshot"
                        );

                        snapshot.upload = delta_upload;
                        snapshot.download = delta_download;
                        Some(snapshot)
                    })
                    .collect();

                let delta_count = delta_snapshots.len();
                if delta_count == 0 {
                    debug!(snapshot_count, "Traffic flush tick: all snapshots filtered out after delta calculation");
                    continue;
                }

                match taos_repo.flush_snapshots(delta_snapshots, ::time::OffsetDateTime::now_utc()).await {
                    Ok(count) => {
                        info!(snapshot_count, delta_count, flushed = count, "Flushed traffic delta snapshots to taos");
                    }
                    Err(e) => {
                        warn!(snapshot_count, delta_count, "Failed to flush traffic delta snapshots to taos: {e:#}");
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutting down");
                break;
            }
        }
    }

    Ok(())
}
