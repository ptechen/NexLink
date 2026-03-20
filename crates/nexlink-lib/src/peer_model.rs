use libp2p::PeerId;
use nexlink_core::{Peer, PeerStatus};
use time::OffsetDateTime;

pub fn connected_peer(peer_id: PeerId) -> Peer {
    Peer {
        peer_id: peer_id.to_string(),
        name: None,
        labels: serde_json::json!({}),
        status: PeerStatus::Online,
        last_seen_at: Some(OffsetDateTime::now_utc()),
    }
}

pub fn disconnected_peer(peer_id: PeerId) -> Peer {
    Peer {
        peer_id: peer_id.to_string(),
        name: None,
        labels: serde_json::json!({}),
        status: PeerStatus::Offline,
        last_seen_at: Some(OffsetDateTime::now_utc()),
    }
}
