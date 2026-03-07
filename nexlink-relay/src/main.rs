use std::time::Duration;

use anyhow::Result;
use nexlink_lib::config::default_data_dir;
use nexlink_lib::identity::NodeIdentity;
use nexlink_lib::network::behaviour::RelayBehaviourEvent;
use nexlink_lib::network::swarm::build_relay_swarm;
use clap::Parser;
use libp2p::futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::Multiaddr;
use tokio::signal;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "nexlink-relay", about = "NexLink P2P relay/rendezvous server")]
struct Cli {
    /// Listen address
    #[arg(short, long, default_value = "/ip4/0.0.0.0/udp/4001/quic-v1")]
    listen: String,

    /// Max concurrent relay reservations
    #[arg(long, default_value_t = 128)]
    max_reservations: usize,

    /// Max circuits per peer
    #[arg(long, default_value_t = 4)]
    max_circuits_per_peer: usize,

    /// Max circuit duration in seconds
    #[arg(long, default_value_t = 300)]
    max_circuit_duration_secs: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let data_dir = default_data_dir().join("relay");
    let identity_path = data_dir.join("identity.json");
    let identity = NodeIdentity::load_or_generate_with_recovery(&identity_path)?;

    info!(peer_id = %identity.peer_id(), "Starting nexlink relay server");

    let relay_config = libp2p::relay::Config {
        max_reservations: cli.max_reservations,
        max_circuits_per_peer: cli.max_circuits_per_peer,
        max_circuit_duration: Duration::from_secs(cli.max_circuit_duration_secs),
        ..Default::default()
    };

    let mut swarm = build_relay_swarm(&identity, relay_config).await?;

    let listen_addr: Multiaddr = cli.listen.parse()?;
    swarm.listen_on(listen_addr)?;

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Relay listening on {}/p2p/{}", address, swarm.local_peer_id());
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        info!(%peer_id, "Peer connected");
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        info!(%peer_id, "Peer disconnected");
                    }
                    SwarmEvent::Behaviour(event) => {
                        match event {
                            RelayBehaviourEvent::RendezvousServer(e) => {
                                info!("Rendezvous: {e:?}");
                            }
                            RelayBehaviourEvent::Relay(e) => {
                                info!("Relay: {e:?}");
                            }
                            RelayBehaviourEvent::Autonat(e) => {
                                info!("AutoNAT: {e:?}");
                            }
                            RelayBehaviourEvent::Identify(e) => {
                                info!("Identify: {e:?}");
                            }
                            RelayBehaviourEvent::Ping(e) => {
                                info!("Ping: {e:?}");
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutting down nexlink relay server");
                break;
            }
        }
    }

    Ok(())
}
