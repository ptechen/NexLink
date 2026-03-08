use crate::identity::NodeIdentity;
use anyhow::Result;
use libp2p::swarm::Swarm;
use libp2p::{autonat, identify, noise, ping, relay, rendezvous, yamux};
use std::time::Duration;
use libp2p_stream as stream;
use tracing::info;

use super::behaviour::{NexlinkBehaviour, RelayBehaviour};

const PROTOCOL_VERSION: &str = "/nexlink/0.1.0";

/// Build a Swarm for client/provider nodes (uses relay client)
pub async fn build_client_swarm(identity: &NodeIdentity) -> Result<Swarm<NexlinkBehaviour>> {
    let swarm = libp2p::SwarmBuilder::with_existing_identity(identity.keypair().clone())
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_relay_client(noise::Config::new, yamux::Config::default)?
        .with_behaviour(|key, relay_behaviour| {
            Ok(NexlinkBehaviour {
                relay_client: relay_behaviour,
                identify: identify::Behaviour::new(identify::Config::new(
                    PROTOCOL_VERSION.to_string(),
                    key.public(),
                )),
                rendezvous_client: rendezvous::client::Behaviour::new(key.clone()),
                ping: ping::Behaviour::new(ping::Config::default()),
                stream: stream::Behaviour::new(),
                autonat: autonat::Behaviour::new(
                    key.public().to_peer_id(),
                    autonat::Config {
                        boot_delay: Duration::from_secs(10),
                        retry_interval: Duration::from_secs(60),
                        only_global_ips: false,
                        ..Default::default()
                    },
                ),
            })
        })?
        .with_swarm_config(|cfg| {
            cfg.with_idle_connection_timeout(std::time::Duration::from_secs(60))
        })
        .build();

    info!(peer_id = %swarm.local_peer_id(), "Built client swarm");
    Ok(swarm)
}

/// Build a Swarm for the relay/rendezvous server
pub async fn build_relay_swarm(
    identity: &NodeIdentity,
    relay_config: relay::Config,
) -> Result<Swarm<RelayBehaviour>> {
    let swarm = libp2p::SwarmBuilder::with_existing_identity(identity.keypair().clone())
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|key| {
            Ok(RelayBehaviour {
                relay: relay::Behaviour::new(key.public().to_peer_id(), relay_config),
                identify: identify::Behaviour::new(identify::Config::new(
                    PROTOCOL_VERSION.to_string(),
                    key.public(),
                )),
                rendezvous_server: rendezvous::server::Behaviour::new(
                    rendezvous::server::Config::default(),
                ),
                ping: ping::Behaviour::new(ping::Config::default()),
                stream: stream::Behaviour::new(),
                autonat: autonat::Behaviour::new(
                    key.public().to_peer_id(),
                    autonat::Config {
                        only_global_ips: false,
                        ..Default::default()
                    },
                ),
            })
        })?
        .with_swarm_config(|cfg| {
            cfg.with_idle_connection_timeout(std::time::Duration::from_secs(120))
        })
        .build();

    info!(peer_id = %swarm.local_peer_id(), "Built relay swarm");
    Ok(swarm)
}
