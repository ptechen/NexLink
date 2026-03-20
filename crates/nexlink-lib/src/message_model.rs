use nexlink_core::{
    inbound_connector_event, inbound_event, outbound_event, InboundConnectorMessage,
    InboundMessageContext, MessageInboundPayload, MessageOutboundPayload, OutboundMessageContext,
};
use time::OffsetDateTime;

pub fn inbound_message_event(
    event_id: impl Into<String>,
    source_peer: Option<String>,
    target_peer: Option<String>,
    session_key: impl Into<String>,
    channel: impl Into<String>,
    payload: MessageInboundPayload,
) -> nexlink_core::EventEnvelope {
    inbound_event(InboundMessageContext {
        event_id: event_id.into(),
        source_peer,
        target_peer,
        session_key: session_key.into(),
        channel: channel.into(),
        payload,
        created_at: OffsetDateTime::now_utc(),
    })
}

pub fn connector_inbound_event(
    event_id: impl Into<String>,
    source_peer: Option<String>,
    target_peer: Option<String>,
    session_key: impl Into<String>,
    channel: impl Into<String>,
    message_id: impl Into<String>,
    sender_id: impl Into<String>,
    text: Option<String>,
    attachments: Vec<nexlink_core::Attachment>,
    metadata: serde_json::Value,
) -> nexlink_core::EventEnvelope {
    inbound_connector_event(InboundConnectorMessage {
        event_id: event_id.into(),
        session_key: session_key.into(),
        channel: channel.into(),
        source_peer,
        target_peer,
        message_id: message_id.into(),
        sender_id: sender_id.into(),
        text,
        attachments,
        metadata,
        created_at: OffsetDateTime::now_utc(),
    })
}

pub fn outbound_message_event(
    event_id: impl Into<String>,
    source_peer: Option<String>,
    target_peer: Option<String>,
    session_key: impl Into<String>,
    channel: impl Into<String>,
    payload: MessageOutboundPayload,
) -> nexlink_core::EventEnvelope {
    outbound_event(OutboundMessageContext {
        event_id: event_id.into(),
        source_peer,
        target_peer,
        session_key: session_key.into(),
        channel: channel.into(),
        payload,
        created_at: OffsetDateTime::now_utc(),
    })
}
