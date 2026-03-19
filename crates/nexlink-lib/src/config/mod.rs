use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Address to listen on for libp2p (e.g. "/ip4/0.0.0.0/udp/0/quic-v1")
    pub listen_addr: String,
    /// Relay server multiaddr for rendezvous and relay
    pub relay_addr: Option<String>,
    /// Namespace for rendezvous (private network ID or "nexlink-public")
    pub namespace: String,
    /// Local unified proxy listen port (handles both SOCKS5 and HTTP CONNECT)
    pub unified_port: u16,
    /// Whether this node serves as a provider (proxy service provider)
    pub provider: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/udp/0/quic-v1".to_string(),
            relay_addr: None,
            namespace: "nexlink-public".to_string(),
            unified_port: 7890, // Default unified port
            provider: false,
        }
    }
}

/// Returns the default data directory: ~/.nexlink/
pub fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Cannot determine home directory")
        .join(".nexlink")
}

/// Returns the default identity file path: ~/.nexlink/identity.json
pub fn default_identity_path() -> PathBuf {
    default_data_dir().join("identity.json")
}
