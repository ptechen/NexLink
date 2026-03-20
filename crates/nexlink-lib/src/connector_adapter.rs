use async_trait::async_trait;
use nexlink_core::{Attachment, EventEnvelope};

use crate::connector_envelope::ConnectorEnvelopeBuilder;

#[derive(Debug, Clone)]
pub struct ConnectorInboundInput {
    pub event_id: String,
    pub session_key: String,
    pub message_id: String,
    pub sender_id: String,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ConnectorOutboundInput {
    pub event_id: String,
    pub session_key: String,
    pub reply_to: Option<String>,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
}

#[async_trait]
pub trait ConnectorAdapter: Send + Sync {
    fn envelope_builder(&self) -> &ConnectorEnvelopeBuilder;

    async fn map_inbound(&self, input: ConnectorInboundInput) -> anyhow::Result<EventEnvelope> {
        Ok(self.envelope_builder().inbound_connector(
            input.event_id,
            input.session_key,
            input.message_id,
            input.sender_id,
            input.text,
            input.attachments,
            input.metadata,
        ))
    }

    async fn map_outbound(&self, input: ConnectorOutboundInput) -> anyhow::Result<EventEnvelope> {
        Ok(self.envelope_builder().outbound_connector(
            input.event_id,
            input.session_key,
            input.reply_to,
            input.text,
            input.attachments,
            input.metadata,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexlink_core::{EventPayload, EventType};

    struct FakeAdapter {
        builder: ConnectorEnvelopeBuilder,
    }

    #[async_trait]
    impl ConnectorAdapter for FakeAdapter {
        fn envelope_builder(&self) -> &ConnectorEnvelopeBuilder {
            &self.builder
        }
    }

    #[tokio::test]
    async fn maps_inbound_input_to_event() {
        let adapter = FakeAdapter {
            builder: ConnectorEnvelopeBuilder::new("qqbot").source_peer("peer-a"),
        };
        let event = adapter
            .map_inbound(ConnectorInboundInput {
                event_id: "evt-1".into(),
                session_key: "qqbot:c2c:1".into(),
                message_id: "msg-1".into(),
                sender_id: "user-1".into(),
                text: Some("hello".into()),
                attachments: vec![],
                metadata: serde_json::json!({"surface": "qq"}),
            })
            .await
            .unwrap();

        assert!(matches!(event.event_type, EventType::MessageInbound));
        match event.payload {
            EventPayload::MessageInbound(payload) => {
                assert_eq!(payload.sender_id, "user-1");
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[tokio::test]
    async fn maps_outbound_input_to_event() {
        let adapter = FakeAdapter {
            builder: ConnectorEnvelopeBuilder::new("telegram").target_peer("peer-b"),
        };
        let event = adapter
            .map_outbound(ConnectorOutboundInput {
                event_id: "evt-2".into(),
                session_key: "telegram:chat:1".into(),
                reply_to: Some("msg-9".into()),
                text: Some("world".into()),
                attachments: vec![],
                metadata: serde_json::json!({"surface": "telegram"}),
            })
            .await
            .unwrap();

        assert!(matches!(event.event_type, EventType::MessageOutbound));
        match event.payload {
            EventPayload::MessageOutbound(payload) => {
                assert_eq!(payload.reply_to.as_deref(), Some("msg-9"));
                assert_eq!(payload.text.as_deref(), Some("world"));
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }
}
