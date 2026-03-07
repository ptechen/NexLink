use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::Path;

const SALT: &[u8] = b"nexlink-network-v1";
const INFO: &[u8] = b"rendezvous-namespace";

/// Derive a deterministic NetworkId from name + password via HKDF-SHA256.
pub fn derive_network_id(name: &str, password: &str) -> String {
    let ikm = format!("{name}:{password}");
    let hk = Hkdf::<Sha256>::new(Some(SALT), ikm.as_bytes());
    let mut okm = [0u8; 16];
    hk.expand(INFO, &mut okm).expect("16 bytes is valid for HKDF");
    hex::encode(okm)
}

/// Persistent network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// "public" or "private"
    pub mode: String,
    /// Human-readable network name (only for private)
    pub network_name: Option<String>,
    /// Derived network ID hex (only for private)
    pub network_id: Option<String>,
    /// Rendezvous namespace
    pub namespace: String,
    /// Relay server multiaddr
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_addr: Option<String>,
    /// Last selected provider PeerId
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "last_exit_node")]
    pub last_provider: Option<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: "public".to_string(),
            network_name: None,
            network_id: None,
            namespace: "nexlink-public".to_string(),
            relay_addr: None,
            last_provider: None,
        }
    }
}

impl NetworkConfig {
    pub fn public() -> Self {
        Self::default()
    }

    pub fn private(name: &str, password: &str) -> Self {
        let network_id = derive_network_id(name, password);
        let namespace = format!("nexlink-{network_id}");
        Self {
            mode: "private".to_string(),
            network_name: Some(name.to_string()),
            network_id: Some(network_id),
            namespace,
            relay_addr: None,
            last_provider: None,
        }
    }

    pub fn is_private(&self) -> bool {
        self.mode == "private"
    }
}

const CONFIG_FILE: &str = "network.json";

pub fn load_network_config(data_dir: &Path) -> NetworkConfig {
    let path = data_dir.join(CONFIG_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => NetworkConfig::default(),
    }
}

pub fn save_network_config(data_dir: &Path, config: &NetworkConfig) -> anyhow::Result<()> {
    let path = data_dir.join(CONFIG_FILE);
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn remove_network_config(data_dir: &Path) {
    let path = data_dir.join(CONFIG_FILE);
    let _ = std::fs::remove_file(path);
}
