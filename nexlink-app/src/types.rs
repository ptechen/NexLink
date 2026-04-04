use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub peer_id: String,
    pub addrs: Vec<String>,
    pub is_provider: bool,
    pub latency_ms: Option<u64>,
    pub connected: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub upload_speed: u64,
    pub download_speed: u64,
    pub active_connections: u32,
    #[serde(default)]
    pub quota_available: bool,
    #[serde(default)]
    pub total_used: u64,
    #[serde(default)]
    pub total_limit: u64,
    #[serde(default)]
    pub remaining_bytes: u64,
    #[serde(default)]
    pub usage_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub running: bool,
    pub unified_port: u16,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub peer_id: String,
    pub proxy_status: Option<ProxyStatus>,
    pub connected_peer: Option<String>,
    pub discovered_peers: Vec<PeerInfo>,
    pub nat_status: String,
    pub traffic: TrafficStats,
    pub relay_addr: String,
    pub namespace: String,
    pub data_dir: String,
    pub network_mode: String,
    pub network_name: Option<String>,
    #[serde(default)]
    pub system_proxy_enabled: bool,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
}
