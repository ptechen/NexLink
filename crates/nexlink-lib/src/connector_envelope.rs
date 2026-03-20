use nexlink_core::{
    Attachment, EventEnvelope, MessageInboundPayload, MessageOutboundPayload,
    OutboundConnectorMessage,
};

use crate::message_model::{
    connector_inbound_event, connector_outbound_event, inbound_message_event,
    outbound_message_event,
};

#[derive(Debug, Clone, Default)]
pub struct ConnectorEnvelopeBuilder {
    pub source_peer: Option<String>,
    pub target_peer: Option<String>,
    pub channel: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexlink_core::{EventPayload, EventType};

    #[test]
    fn builds_inbound_connector_event() {
        let builder = ConnectorEnvelopeBuilder::new("qqbot")
            .source_peer("peer-a")
            .target_peer("peer-b");
        let event = builder.inbound_connector(
            "evt-1",
            "qqbot:c2c:123",
            "msg-1",
            "user-1",
            Some("hello".to_string()),
            vec![],
            serde_json::json!({"surface": "qqbot"}),
        );

        assert!(matches!(event.event_type, EventType::MessageInbound));
        assert_eq!(event.channel.as_deref(), Some("qqbot"));
        assert_eq!(event.session_key.as_deref(), Some("qqbot:c2c:123"));
        match event.payload {
            EventPayload::MessageInbound(payload) => {
                assert_eq!(payload.message_id, "msg-1");
                assert_eq!(payload.sender_id, "user-1");
                assert_eq!(payload.text.as_deref(), Some("hello"));
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn builds_outbound_connector_event() {
        let builder = ConnectorEnvelopeBuilder::new("telegram").source_peer("peer-b");
        let event = builder.outbound_connector(
            "evt-2",
            "telegram:chat:456",
            Some("msg-2".to_string()),
            Some("world".to_string()),
            vec![],
            serde_json::json!({"surface": "telegram"}),
        );

        assert!(matches!(event.event_type, EventType::MessageOutbound));
        assert_eq!(event.channel.as_deref(), Some("telegram"));
        assert_eq!(event.session_key.as_deref(), Some("telegram:chat:456"));
        match event.payload {
            EventPayload::MessageOutbound(payload) => {
                assert_eq!(payload.reply_to.as_deref(), Some("msg-2"));
                assert_eq!(payload.text.as_deref(), Some("world"));
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }
}

impl ConnectorEnvelopeBuilder {
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            source_peer: None,
            target_peer: None,
            channel: channel.into(),
        }
    }

    pub fn source_peer(mut self, peer: impl Into<String>) -> Self {
        self.source_peer = Some(peer.into());
        self
    }

    pub fn target_peer(mut self, peer: impl Into<String>) -> Self {
        self.target_peer = Some(peer.into());
        self
    }

    pub fn inbound_payload(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        payload: MessageInboundPayload,
    ) -> EventEnvelope {
        inbound_message_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            payload,
        )
    }

    pub fn inbound_connector(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        message_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: Option<String>,
        attachments: Vec<Attachment>,
        metadata: serde_json::Value,
    ) -> EventEnvelope {
        connector_inbound_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            message_id,
            sender_id,
            text,
            attachments,
            metadata,
        )
    }

    pub fn outbound_payload(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        payload: MessageOutboundPayload,
    ) -> EventEnvelope {
        outbound_message_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            payload,
        )
    }

    pub fn outbound_connector(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        reply_to: Option<String>,
        text: Option<String>,
        attachments: Vec<Attachment>,
        metadata: serde_json::Value,
    ) -> EventEnvelope {
        connector_outbound_event(
            event_id,
            self.source_peer.clone(),
            self.target_peer.clone(),
            session_key,
            self.channel.clone(),
            reply_to,
            text,
            attachments,
            metadata,
        )
    }

    pub fn outbound_connector_message(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        reply_to: Option<String>,
        text: Option<String>,
        attachments: Vec<Attachment>,
        metadata: serde_json::Value,
    ) -> OutboundConnectorMessage {
        OutboundConnectorMessage {
            event_id: event_id.into(),
            session_key: session_key.into(),
            channel: self.channel.clone(),
            source_peer: self.source_peer.clone(),
            target_peer: self.target_peer.clone(),
            reply_to,
            text,
            attachments,
            metadata,
            created_at: time::OffsetDateTime::now_utc(),
        }
    }
}
