use nexlink_core::{Attachment, EventEnvelope};

use crate::connector_adapter::{ConnectorAdapter, ConnectorInboundInput, ConnectorOutboundInput};
use crate::connector_envelope::ConnectorEnvelopeBuilder;

pub struct QqLikeConnector {
    builder: ConnectorEnvelopeBuilder,
}

impl QqLikeConnector {
    pub fn new(source_peer: impl Into<String>, target_peer: impl Into<String>) -> Self {
        Self {
            builder: ConnectorEnvelopeBuilder::new("qqbot")
                .source_peer(source_peer)
                .target_peer(target_peer),
        }
    }
}

#[async_trait::async_trait]
impl ConnectorAdapter for QqLikeConnector {
    fn envelope_builder(&self) -> &ConnectorEnvelopeBuilder {
        &self.builder
    }
}

pub struct TelegramLikeConnector {
    builder: ConnectorEnvelopeBuilder,
}

impl TelegramLikeConnector {
    pub fn new(source_peer: impl Into<String>, target_peer: impl Into<String>) -> Self {
        Self {
            builder: ConnectorEnvelopeBuilder::new("telegram")
                .source_peer(source_peer)
                .target_peer(target_peer),
        }
    }
}

#[async_trait::async_trait]
impl ConnectorAdapter for TelegramLikeConnector {
    fn envelope_builder(&self) -> &ConnectorEnvelopeBuilder {
        &self.builder
    }
}

pub async fn qq_like_inbound_example(
    source_peer: impl Into<String>,
    target_peer: impl Into<String>,
) -> anyhow::Result<EventEnvelope> {
    let connector = QqLikeConnector::new(source_peer, target_peer);
    connector
        .map_inbound(ConnectorInboundInput {
            event_id: "evt-qq-1".into(),
            session_key: "qqbot:c2c:demo".into(),
            message_id: "msg-qq-1".into(),
            sender_id: "user-qq-1".into(),
            text: Some("hello from qq".into()),
            attachments: vec![],
            metadata: serde_json::json!({"surface": "qqbot-demo"}),
        })
        .await
}

pub async fn telegram_like_outbound_example(
    source_peer: impl Into<String>,
    target_peer: impl Into<String>,
) -> anyhow::Result<EventEnvelope> {
    let connector = TelegramLikeConnector::new(source_peer, target_peer);
    connector
        .map_outbound(ConnectorOutboundInput {
            event_id: "evt-tg-1".into(),
            session_key: "telegram:chat:demo".into(),
            reply_to: Some("msg-tg-1".into()),
            text: Some("hello to telegram".into()),
            attachments: Vec::<Attachment>::new(),
            metadata: serde_json::json!({"surface": "telegram-demo"}),
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexlink_core::{EventPayload, EventType};

    #[tokio::test]
    async fn builds_qq_like_inbound_example() {
        let event = qq_like_inbound_example("peer-a", "peer-b").await.unwrap();
        assert!(matches!(event.event_type, EventType::MessageInbound));
        match event.payload {
            EventPayload::MessageInbound(payload) => {
                assert_eq!(payload.sender_id, "user-qq-1");
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[tokio::test]
    async fn builds_telegram_like_outbound_example() {
        let event = telegram_like_outbound_example("peer-b", "peer-c")
            .await
            .unwrap();
        assert!(matches!(event.event_type, EventType::MessageOutbound));
        match event.payload {
            EventPayload::MessageOutbound(payload) => {
                assert_eq!(payload.reply_to.as_deref(), Some("msg-tg-1"));
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }
}
