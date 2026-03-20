use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_key: String,
    pub bot_id: Option<String>,
    pub channel: String,
    pub session_type: SessionType,
    pub status: SessionStatus,
    pub state: serde_json::Value,
    pub last_event_at: Option<time::OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Chat,
    Thread,
    Task,
    Cron,
    AgentRun,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Idle,
    Closed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionParticipant {
    pub session_key: String,
    pub participant_type: ParticipantType,
    pub participant_id: String,
    pub role: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantType {
    User,
    Bot,
    Peer,
    ChannelEntity,
}
