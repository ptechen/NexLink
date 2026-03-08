use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use nexlink_lib::config::default_data_dir;
use nexlink_lib::identity::NodeIdentity;
use nexlink_lib::network::behaviour::RelayBehaviourEvent;
use nexlink_lib::network::swarm::build_relay_swarm;
use nexlink_lib::proxy::{credentials::derive_credentials, CREDENTIALS_PROTOCOL, CREDENTIALS_SYNC_PROTOCOL};
use clap::Parser;
use libp2p::futures::{AsyncWriteExt, StreamExt};
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId};
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{info, warn};
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

    /// Secret for deriving proxy credentials (hex string)
    #[arg(long, default_value = "nexlink-default-secret")]
    credentials_secret: String,
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

    // Track connected peers for credentials sync
    let connected_peers: Arc<RwLock<HashSet<PeerId>>> = Arc::new(RwLock::new(HashSet::new()));

    // Spawn credentials handler (per-peer credential issuance)
    let secret = cli.credentials_secret.as_bytes().to_vec();
    let mut credentials_control = swarm.behaviour().stream.new_control();
    let mut incoming = credentials_control
        .accept(CREDENTIALS_PROTOCOL)
        .expect("credentials protocol not yet registered");

    let secret_for_creds = secret.clone();
    tokio::spawn(async move {
        while let Some((peer_id, mut stream)) = incoming.next().await {
            let creds = derive_credentials(&peer_id, &secret_for_creds);
            let json = match serde_json::to_vec(&creds) {
                Ok(j) => j,
                Err(e) => {
                    warn!(%peer_id, "Failed to serialize credentials: {e}");
                    continue;
                }
            };
            if let Err(e) = stream.write_all(&json).await {
                warn!(%peer_id, "Failed to write credentials: {e}");
                continue;
            }
            if let Err(e) = stream.close().await {
                warn!(%peer_id, "Failed to close credentials stream: {e}");
            }
            info!(%peer_id, username = %creds.username, "Issued proxy credentials");
        }
    });

    // Spawn credentials sync handler (returns all connected peers' credentials)
    let mut sync_control = swarm.behaviour().stream.new_control();
    let mut sync_incoming = sync_control
        .accept(CREDENTIALS_SYNC_PROTOCOL)
        .expect("credentials sync protocol not yet registered");

    let peers_for_sync = connected_peers.clone();
    let secret_for_sync = secret.clone();
    tokio::spawn(async move {
        while let Some((requester, mut stream)) = sync_incoming.next().await {
            let peers = peers_for_sync.read().await;
            let all_creds: Vec<nexlink_lib::proxy::ProxyCredentials> = peers
                .iter()
                .map(|pid| derive_credentials(pid, &secret_for_sync))
                .collect();
            drop(peers);

            let json = match serde_json::to_vec(&all_creds) {
                Ok(j) => j,
                Err(e) => {
                    warn!(%requester, "Failed to serialize credentials sync: {e}");
                    continue;
                }
            };
            if let Err(e) = stream.write_all(&json).await {
                warn!(%requester, "Failed to write credentials sync: {e}");
                continue;
            }
            if let Err(e) = stream.close().await {
                warn!(%requester, "Failed to close credentials sync stream: {e}");
            }
            info!(%requester, count = all_creds.len(), "Synced credentials to provider");
        }
    });

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
                        connected_peers.write().await.insert(peer_id);
                        info!(%peer_id, "Peer connected");
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        connected_peers.write().await.remove(&peer_id);
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
                            RelayBehaviourEvent::Stream(_) => {}
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
