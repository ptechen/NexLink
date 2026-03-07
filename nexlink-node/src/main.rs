use anyhow::Result;
use nexlink_lib::config::default_data_dir;
use nexlink_lib::identity::NodeIdentity;
use nexlink_lib::network::behaviour::NexlinkBehaviourEvent;
use nexlink_lib::network::swarm::build_client_swarm;
use clap::Parser;
use libp2p::futures::StreamExt;
use libp2p::rendezvous;
use libp2p::swarm::SwarmEvent;
use libp2p::Multiaddr;
use libp2p::{autonat, relay};
use std::time::Duration;
use tokio::{signal, time};
use tracing::{info, warn};
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

    info!(peer_id = %identity.peer_id(), "Starting nexlink node");

    let mut swarm = build_client_swarm(&identity).await?;

    // Get a stream::Control handle for proxy operations
    let stream_control = swarm.behaviour().stream.new_control();

    // If running as provider, spawn the incoming proxy stream handler
    if cli.provider {
        let mut accept_control = stream_control.clone();
        let mut incoming = accept_control
            .accept(nexlink_lib::proxy::PROXY_PROTOCOL)
            .expect("proxy protocol not yet registered");

        tokio::spawn(async move {
            use libp2p::futures::StreamExt;
            while let Some((peer_id, stream)) = incoming.next().await {
                tokio::spawn(async move {
                    if let Err(e) =
                        nexlink_lib::proxy::provider_handler::handle_proxy_stream(peer_id, stream, None).await
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
    let relay_circuit_addr: Multiaddr =
        format!("{relay_addr}/p2p-circuit").parse()?;
    swarm.listen_on(relay_circuit_addr)?;

    let namespace =
        rendezvous::Namespace::new(cli.namespace.clone()).expect("Invalid namespace");
    let mut registered = false;
    let mut proxy_started = false;
    let mut discover_interval = time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!(%address, "Listening");
                        // Add actual listen addresses as external so rendezvous advertises them
                        swarm.add_external_address(address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        info!(%peer_id, "Connected to peer");
                    }
                    SwarmEvent::Behaviour(NexlinkBehaviourEvent::RendezvousClient(event)) => {
                        match event {
                            rendezvous::client::Event::Registered { namespace, .. } => {
                                info!(?namespace, "Registered with rendezvous");
                                registered = true;
                                // Discover other peers immediately
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
                                            proxy_started = true;
                                            let unified_ctl = stream_control.clone();
                                            let unified_port = cli.unified_port;
                                            let unified_traffic = nexlink_lib::traffic::TrafficCounter::new();
                                            tokio::spawn(async move {
                                                if let Err(e) = nexlink_lib::proxy::unified_proxy::start_unified_proxy(unified_port, peer, unified_ctl, unified_traffic).await {
                                                    warn!("Unified proxy error: {e:#}");
                                                }
                                            });
                                            info!(unified_port, %peer, "Started unified local proxy -> provider");
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
                        info!("Ping: {event:?}");
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
                                // Provider: register with rendezvous so clients can discover us
                                match swarm.behaviour_mut().rendezvous_client.register(
                                    namespace.clone(),
                                    relay_peer_id,
                                    None,
                                ) {
                                    Ok(()) => info!("Registering with rendezvous"),
                                    Err(e) => warn!("Failed to register: {e}"),
                                }
                            } else {
                                // Client: skip registration, just discover providers
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
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        if let Some(peer) = peer_id {
                            let err_str = error.to_string();
                            warn!(%peer, "Outgoing connection failed: {error}");
                            // Only retry via relay if:
                            // 1. Not the relay itself
                            // 2. The error is not already from a relay circuit (avoid loops)
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
