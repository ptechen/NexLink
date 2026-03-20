use serde::{Deserialize, Serialize};

use crate::{
    EventEnvelope, EventPayload, EventType, MessageInboundPayload, MessageOutboundPayload,
};

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
