use nexlink_core::{Attachment, Runtime};
use std::sync::Arc;

use crate::connector_adapter::{
    deliver_inbound_to_runtime, deliver_outbound_to_runtime, ConnectorAdapter,
    ConnectorInboundInput, ConnectorOutboundInput,
};
use crate::connector_envelope::ConnectorEnvelopeBuilder;

pub struct PlatformConnector<R> {
    builder: ConnectorEnvelopeBuilder,
    runtime: Arc<R>,
}

impl<R> PlatformConnector<R>
where
    R: Runtime + Send + Sync,
{
    pub fn new(channel: impl Into<String>, runtime: Arc<R>) -> Self {
        Self {
            builder: ConnectorEnvelopeBuilder::new(channel),
            runtime,
        }
    }

    pub fn source_peer(mut self, peer: impl Into<String>) -> Self {
        self.builder = self.builder.source_peer(peer);
        self
    }

    pub fn target_peer(mut self, peer: impl Into<String>) -> Self {
        self.builder = self.builder.target_peer(peer);
        self
    }

    pub async fn handle_inbound(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        message_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: Option<String>,
        attachments: Vec<Attachment>,
        metadata: serde_json::Value,
    ) -> anyhow::Result<()> {
        deliver_inbound_to_runtime(
            self.runtime.as_ref(),
            self,
            ConnectorInboundInput {
                event_id: event_id.into(),
                session_key: session_key.into(),
                message_id: message_id.into(),
                sender_id: sender_id.into(),
                text,
                attachments,
                metadata,
            },
        )
        .await
    }

    pub async fn handle_outbound(
        &self,
        event_id: impl Into<String>,
        session_key: impl Into<String>,
        reply_to: Option<String>,
        text: Option<String>,
        attachments: Vec<Attachment>,
        metadata: serde_json::Value,
    ) -> anyhow::Result<()> {
        deliver_outbound_to_runtime(
            self.runtime.as_ref(),
            self,
            ConnectorOutboundInput {
                event_id: event_id.into(),
                session_key: session_key.into(),
                reply_to,
                text,
                attachments,
                metadata,
            },
        )
        .await
    }
}

#[async_trait::async_trait]
impl<R> ConnectorAdapter for PlatformConnector<R>
where
    R: Runtime + Send + Sync,
{
    fn envelope_builder(&self) -> &ConnectorEnvelopeBuilder {
        &self.builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexlink_core::{EventEnvelope, InvokeRequest, InvokeResponse, InvokeStatus};
    use std::sync::Mutex;

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
    async fn platform_connector_handles_inbound() {
        let runtime = Arc::new(FakeRuntime {
            events: Mutex::new(Vec::new()),
        });
        let connector = PlatformConnector::new("qqbot", runtime.clone())
            .source_peer("peer-a")
            .target_peer("peer-b");
        connector
            .handle_inbound(
                "evt-1",
                "qqbot:c2c:test",
                "msg-1",
                "user-1",
                Some("hello".into()),
                vec![],
                serde_json::json!({"surface": "qqbot"}),
            )
            .await
            .unwrap();
        assert_eq!(runtime.events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn platform_connector_handles_outbound() {
        let runtime = Arc::new(FakeRuntime {
            events: Mutex::new(Vec::new()),
        });
        let connector = PlatformConnector::new("telegram", runtime.clone())
            .source_peer("peer-b")
            .target_peer("peer-c");
        connector
            .handle_outbound(
                "evt-2",
                "telegram:chat:test",
                Some("msg-2".into()),
                Some("world".into()),
                vec![],
                serde_json::json!({"surface": "telegram"}),
            )
            .await
            .unwrap();
        assert_eq!(runtime.events.lock().unwrap().len(), 1);
    }
}
