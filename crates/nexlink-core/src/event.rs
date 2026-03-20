use serde::{Deserialize, Serialize};

use crate::{InvokeRequest, InvokeResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: String,
    pub event_type: EventType,
    pub source_peer: Option<String>,
    pub target_peer: Option<String>,
    pub session_key: Option<String>,
    pub channel: Option<String>,
    pub created_at: time::OffsetDateTime,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    MessageInbound,
    MessageOutbound,
    ToolInvoke,
    ToolResult,
    CapabilityRegister,
    CapabilityHeartbeat,
    SessionOpened,
    SessionUpdated,
    PeerHeartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum EventPayload {
    MessageInbound(MessageInboundPayload),
    MessageOutbound(MessageOutboundPayload),
    ToolInvoke(InvokeRequest),
    ToolResult(InvokeResponse),
    SessionUpdated(SessionUpdatedPayload),
    Generic(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub url: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInboundPayload {
    pub message_id: String,
    pub sender_id: String,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageOutboundPayload {
    pub reply_to: Option<String>,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdatedPayload {
    pub session_key: String,
    pub patch: serde_json::Value,
}
