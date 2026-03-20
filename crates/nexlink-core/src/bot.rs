use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bot {
    pub bot_id: String,
    pub peer_id: String,
    pub name: String,
    pub runtime: String,
    pub enabled: bool,
    pub config: serde_json::Value,
}
