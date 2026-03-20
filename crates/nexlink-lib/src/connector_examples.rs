use nexlink_core::{
    Attachment, EventEnvelope, InvokeRequest, InvokeResponse, InvokeStatus, Runtime,
};
use std::sync::{Arc, Mutex};

use crate::connector_adapter::{
    deliver_inbound_to_runtime, ConnectorAdapter, ConnectorInboundInput, ConnectorOutboundInput,
};
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

pub struct RuntimeBridge<R> {
    runtime: Arc<R>,
}

impl<R> RuntimeBridge<R>
where
    R: Runtime,
{
    pub fn new(runtime: Arc<R>) -> Self {
        Self { runtime }
    }

    pub async fn deliver(&self, event: EventEnvelope) -> anyhow::Result<()> {
        self.runtime.handle_event(event).await
    }
}

pub async fn drive_qq_like_inbound_to_runtime<R>(
    runtime: Arc<R>,
    source_peer: impl Into<String>,
    target_peer: impl Into<String>,
) -> anyhow::Result<()>
where
    R: Runtime + Send + Sync,
{
    let connector = QqLikeConnector::new(source_peer, target_peer);
    deliver_inbound_to_runtime(
        runtime.as_ref(),
        &connector,
        ConnectorInboundInput {
            event_id: "evt-qq-drive-1".into(),
            session_key: "qqbot:c2c:drive".into(),
            message_id: "msg-qq-drive-1".into(),
            sender_id: "user-qq-drive-1".into(),
            text: Some("hello runtime".into()),
            attachments: vec![],
            metadata: serde_json::json!({"surface": "qqbot-drive"}),
        },
    )
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

    struct FakeRuntime {
        events: Mutex<Vec<EventEnvelope>>,
    }

    #[async_trait::async_trait]
    impl Runtime for FakeRuntime {
        async fn handle_event(&self, event: EventEnvelope) -> anyhow::Result<()> {
            self.events.lock().unwrap().push(event);
            Ok(())
        }

        async fn invoke_capability(
            &self,
            request: InvokeRequest,
        ) -> anyhow::Result<InvokeResponse> {
            Ok(InvokeResponse {
                request_id: request.request_id,
                status: InvokeStatus::Succeeded,
                result: serde_json::json!({}),
                error: None,
            })
        }
    }

    #[tokio::test]
    async fn runtime_bridge_delivers_event() {
        let runtime = Arc::new(FakeRuntime {
            events: Mutex::new(Vec::new()),
        });
        let bridge = RuntimeBridge::new(runtime.clone());
        let event = qq_like_inbound_example("peer-a", "peer-b").await.unwrap();
        bridge.deliver(event).await.unwrap();
        assert_eq!(runtime.events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn drives_qq_like_inbound_to_runtime() {
        let runtime = Arc::new(FakeRuntime {
            events: Mutex::new(Vec::new()),
        });
        drive_qq_like_inbound_to_runtime(runtime.clone(), "peer-a", "peer-b")
            .await
            .unwrap();
        assert_eq!(runtime.events.lock().unwrap().len(), 1);
    }
}
