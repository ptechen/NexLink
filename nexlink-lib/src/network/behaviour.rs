use libp2p::swarm::NetworkBehaviour;
use libp2p::{autonat, identify, ping, relay, rendezvous};
use libp2p_stream as stream;

/// Behaviour for client/provider nodes — connects to relay, discovers peers
#[derive(NetworkBehaviour)]
pub struct NexlinkBehaviour {
    pub relay_client: relay::client::Behaviour,
    pub identify: identify::Behaviour,
    pub rendezvous_client: rendezvous::client::Behaviour,
    pub ping: ping::Behaviour,
    pub stream: stream::Behaviour,
    pub autonat: autonat::Behaviour,
}

/// Behaviour for the relay/rendezvous server
#[derive(NetworkBehaviour)]
pub struct RelayBehaviour {
    pub relay: relay::Behaviour,
    pub identify: identify::Behaviour,
    pub rendezvous_server: rendezvous::server::Behaviour,
    pub ping: ping::Behaviour,
    pub autonat: autonat::Behaviour,
}
