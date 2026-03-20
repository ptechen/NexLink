use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub capability_id: String,
    pub peer_id: String,
    pub bot_id: Option<String>,
    pub name: String,
    pub version: String,
    pub scope: CapabilityScope,
    pub visibility: Visibility,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub metadata: serde_json::Value,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityScope {
    Peer,
    Bot,
    Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Shared,
    Public,
}
