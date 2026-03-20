use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    pub request_id: String,
    pub capability: String,
    pub caller_peer: String,
    pub target_peer: Option<String>,
    pub session_key: Option<String>,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResponse {
    pub request_id: String,
    pub status: InvokeStatus,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvokeStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
