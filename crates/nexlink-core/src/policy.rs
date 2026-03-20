use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: String,
    pub subject_type: PolicySubjectType,
    pub subject_id: String,
    pub enabled: bool,
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicySubjectType {
    Peer,
    Bot,
    Session,
    Tenant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub effect: PolicyEffect,
    pub capability: Option<String>,
    pub channel: Option<String>,
    pub target_peer: Option<String>,
    pub conditions: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    Deny,
}
