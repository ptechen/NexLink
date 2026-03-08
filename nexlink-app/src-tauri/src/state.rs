use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub running: bool,
    pub unified_port: u16,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedState {
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
    pub system_proxy_enabled: bool,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
}

#[derive(Debug)]
pub enum AppCommand {
    StartProxy {
        unified_port: u16,
        done: oneshot::Sender<Result<(), String>>,
    },
    StopProxy {
        done: oneshot::Sender<Result<(), String>>,
    },
    ConnectNode {
        peer_id: String,
    },
    DisconnectNode,
    RefreshNodes,
    UpdateConfig {
        relay_addr: Option<String>,
        namespace: Option<String>,
    },
    JoinNetwork {
        name: String,
        password: String,
    },
    LeaveNetwork,
    SetSystemProxy {
        done: oneshot::Sender<Result<(), String>>,
    },
    ClearSystemProxy {
        done: oneshot::Sender<Result<(), String>>,
    },
}

pub struct AppState {
    pub cmd_tx: mpsc::Sender<AppCommand>,
    pub shared: Arc<RwLock<SharedState>>,
}
