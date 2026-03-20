use nexlink_core::Attachment;

#[derive(Debug, Clone)]
pub struct PlatformInboundMessage {
    pub event_id: String,
    pub session_key: String,
    pub message_id: String,
    pub sender_id: String,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct PlatformOutboundMessage {
    pub event_id: String,
    pub session_key: String,
    pub reply_to: Option<String>,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
}
