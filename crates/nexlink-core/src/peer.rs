use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub peer_id: String,
    pub name: Option<String>,
    pub labels: serde_json::Value,
    pub status: PeerStatus,
    pub last_seen_at: Option<time::OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerStatus {
    Online,
    Offline,
    Degraded,
    Unknown,
}
