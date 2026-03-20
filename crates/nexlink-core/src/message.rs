use serde::{Deserialize, Serialize};

use crate::{
    Attachment, EventEnvelope, EventPayload, EventType, MessageInboundPayload,
    MessageOutboundPayload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundConnectorMessage {
    pub event_id: String,
    pub session_key: String,
    pub channel: String,
    pub source_peer: Option<String>,
    pub target_peer: Option<String>,
    pub message_id: String,
    pub sender_id: String,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessageContext {
    pub event_id: String,
    pub source_peer: Option<String>,
    pub target_peer: Option<String>,
    pub session_key: String,
    pub channel: String,
    pub payload: MessageInboundPayload,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessageContext {
    pub event_id: String,
    pub source_peer: Option<String>,
    pub target_peer: Option<String>,
    pub session_key: String,
    pub channel: String,
    pub payload: MessageOutboundPayload,
    pub created_at: time::OffsetDateTime,
}

pub fn inbound_event(ctx: InboundMessageContext) -> EventEnvelope {
    EventEnvelope {
        event_id: ctx.event_id,
        event_type: EventType::MessageInbound,
        source_peer: ctx.source_peer,
        target_peer: ctx.target_peer,
        session_key: Some(ctx.session_key),
        channel: Some(ctx.channel),
        created_at: ctx.created_at,
        payload: EventPayload::MessageInbound(ctx.payload),
    }
}

pub fn inbound_connector_event(msg: InboundConnectorMessage) -> EventEnvelope {
    inbound_event(InboundMessageContext {
        event_id: msg.event_id,
        source_peer: msg.source_peer,
        target_peer: msg.target_peer,
        session_key: msg.session_key,
        channel: msg.channel,
        payload: MessageInboundPayload {
            message_id: msg.message_id,
            sender_id: msg.sender_id,
            text: msg.text,
            attachments: msg.attachments,
            metadata: msg.metadata,
        },
        created_at: msg.created_at,
    })
}

pub fn outbound_event(ctx: OutboundMessageContext) -> EventEnvelope {
    EventEnvelope {
        event_id: ctx.event_id,
        event_type: EventType::MessageOutbound,
        source_peer: ctx.source_peer,
        target_peer: ctx.target_peer,
        session_key: Some(ctx.session_key),
        channel: Some(ctx.channel),
        created_at: ctx.created_at,
        payload: EventPayload::MessageOutbound(ctx.payload),
    }
}
