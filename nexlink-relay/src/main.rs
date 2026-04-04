use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::Parser;
use libp2p::futures::{AsyncWriteExt, StreamExt};
use libp2p::swarm::SwarmEvent;
use libp2p::{ping, Multiaddr, PeerId};
use nexlink_lib::cache::{CONNECTED_PEERS, PEER_CACHE, PROVIDER_PEERS};
use nexlink_lib::config::default_data_dir;
use nexlink_lib::identity::NodeIdentity;
use nexlink_lib::network::behaviour::RelayBehaviourEvent;
use nexlink_lib::network::swarm::build_relay_swarm;
use nexlink_lib::proxy::{
    credentials::derive_credentials, TrafficQuota, CREDENTIALS_PROTOCOL, CREDENTIALS_SYNC_PROTOCOL,
    TRAFFIC_USAGE_PROTOCOL,
};
use nexlink_postgresql::nexlink::peer_user::PeerUser;
use tokio::signal;
use tracing::{debug, info, warn};
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
    #[arg(long)]
    credentials_secret: Option<String>,
}

async fn wait_for_provider_registration(requester: PeerId) -> bool {
    if PROVIDER_PEERS.contains(&requester) {
        return true;
    }

    // The provider can receive the register response before the relay main loop
    // has processed `PeerRegistered` and updated `provider_peers`.
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if PROVIDER_PEERS.contains(&requester) {
            return true;
        }
    }

    false
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
    let credentials_secret = cli
        .credentials_secret
        .or_else(|| std::env::var("NEXLINK_CREDENTIALS_SECRET").ok())
        .ok_or_else(|| anyhow!("missing proxy credentials secret; pass --credentials-secret or set NEXLINK_CREDENTIALS_SECRET"))?;

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

    // Spawn credentials handler (per-peer credential issuance)
    let secret = credentials_secret.into_bytes();
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

    let secret_for_sync = secret.clone();
    tokio::spawn(async move {
        while let Some((requester, mut stream)) = sync_incoming.next().await {
            if !wait_for_provider_registration(requester).await {
                warn!(%requester, "Rejected credentials sync for unregistered provider");
                let _ = stream.close().await;
                continue;
            }
            let all_creds: Vec<nexlink_lib::proxy::ProxyCredentials> = CONNECTED_PEERS
                .iter()
                .filter(|pid| !PROVIDER_PEERS.contains(pid))
                .map(|pid| derive_credentials(&pid, &secret_for_sync))
                .collect();

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

    let mut traffic_control = swarm.behaviour().stream.new_control();
    let mut traffic_incoming = traffic_control
        .accept(TRAFFIC_USAGE_PROTOCOL)
        .expect("traffic usage protocol not yet registered");

    tokio::spawn(async move {
        while let Some((peer_id, mut stream)) = traffic_incoming.next().await {
            let usage = match PeerUser::select_optional_by_peer_id(&peer_id.to_string()).await {
                Ok(Some(peer_user)) => Some(TrafficQuota {
                    total_used: peer_user.send.max(0) as u64 + peer_user.recv.max(0) as u64,
                    total_limit: peer_user.total_limit.max(0) as u64,
                }),
                Ok(None) => None,
                Err(e) => {
                    warn!(%peer_id, "Failed to load peer_user for traffic usage: {e}");
                    continue;
                }
            };

            let json = match serde_json::to_vec(&usage) {
                Ok(value) => value,
                Err(e) => {
                    warn!(%peer_id, "Failed to serialize traffic usage: {e}");
                    continue;
                }
            };

            if let Err(e) = stream.write_all(&json).await {
                warn!(%peer_id, "Failed to write traffic usage: {e}");
                continue;
            }
            if let Err(e) = stream.close().await {
                warn!(%peer_id, "Failed to close traffic usage stream: {e}");
            }
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
                        CONNECTED_PEERS.insert(peer_id);
                        if !PEER_CACHE.contains_key(&peer_id) {
                            tokio::spawn(async move {
                                match PeerUser::insert_if_not_exists(&peer_id.to_string()).await {
                                    Ok(id) => {
                                        PEER_CACHE.insert(peer_id, id);
                                        info!(%peer_id, db_id = id, "Peer cached");
                                    }
                                    Err(e) => {
                                        warn!(%peer_id, "Failed to upsert peer_user: {e}");
                                    }
                                }
                            });
                        }
                        info!(%peer_id, "Peer connected");
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        CONNECTED_PEERS.remove(&peer_id);
                        PROVIDER_PEERS.remove(&peer_id);
                        info!(%peer_id, "Peer disconnected");
                    }
                    SwarmEvent::Behaviour(event) => {
                        match event {
                            RelayBehaviourEvent::RendezvousServer(e) => {
                                match e {
                                    libp2p::rendezvous::server::Event::PeerRegistered { peer, .. } => {
                                        PROVIDER_PEERS.insert(peer);
                                        info!(%peer, "Provider registered with rendezvous");
                                    }
                                    libp2p::rendezvous::server::Event::PeerUnregistered { peer, .. } => {
                                        PROVIDER_PEERS.remove(&peer);
                                        info!(%peer, "Provider unregistered from rendezvous");
                                    }
                                    libp2p::rendezvous::server::Event::RegistrationExpired(registration) => {
                                        let peer = registration.record.peer_id();
                                        PROVIDER_PEERS.remove(&peer);
                                        info!(%peer, "Provider registration expired");
                                    }
                                    other => {
                                        info!("Rendezvous: {other:?}");
                                    }
                                }
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
                                let peer_id = e.peer;
                                let connection = e.connection;
                                match e.result {
                                    Ok(rtt) => {
                                        debug!(%peer_id, ?connection, rtt_ms = rtt.as_millis(), "Ping ok");
                                    }
                                    Err(error) => {
                                        warn!(%peer_id, ?connection, "Ping failed: {error}");
                                        if is_ping_timeout(&error) && swarm.close_connection(connection) {
                                            info!(%peer_id, ?connection, "Closing timed-out ping connection");
                                        }
                                    }
                                }
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
