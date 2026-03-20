use nexlink_core::{
    inbound_event, outbound_event, InboundMessageContext, MessageInboundPayload,
    MessageOutboundPayload, OutboundMessageContext,
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
