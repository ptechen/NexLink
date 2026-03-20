use nexlink_core::{PlatformChannel, Runtime};
use std::sync::Arc;

use crate::platform_connector::PlatformConnector;

pub struct PlatformConnectorFactory<R> {
    runtime: Arc<R>,
}

impl<R> PlatformConnectorFactory<R>
where
    R: Runtime + Send + Sync,
{
    pub fn new(runtime: Arc<R>) -> Self {
        Self { runtime }
    }

    pub fn qqbot(
        &self,
        source_peer: impl Into<String>,
        target_peer: impl Into<String>,
    ) -> PlatformConnector<R> {
        PlatformConnector::new(PlatformChannel::Qqbot.as_str(), self.runtime.clone())
            .source_peer(source_peer)
            .target_peer(target_peer)
    }

    pub fn telegram(
        &self,
        source_peer: impl Into<String>,
        target_peer: impl Into<String>,
    ) -> PlatformConnector<R> {
        PlatformConnector::new("telegram", self.runtime.clone())
            .source_peer(source_peer)
            .target_peer(target_peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexlink_core::{Connector, EventEnvelope, InvokeRequest, InvokeResponse, InvokeStatus};
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
    async fn builds_qqbot_connector_from_factory() {
        let runtime = Arc::new(FakeRuntime {
            events: Mutex::new(Vec::new()),
        });
        let factory = PlatformConnectorFactory::new(runtime.clone());
        let connector = factory.qqbot("peer-a", "peer-b");
        assert_eq!(connector.channel(), "qqbot");
        connector
            .handle_inbound(
                "evt-1",
                "qqbot:c2c:test",
                "msg-1",
                "user-1",
                Some("hello".into()),
                vec![],
                serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(runtime.events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn builds_telegram_connector_from_factory() {
        let runtime = Arc::new(FakeRuntime {
            events: Mutex::new(Vec::new()),
        });
        let factory = PlatformConnectorFactory::new(runtime.clone());
        let connector = factory.telegram("peer-b", "peer-c");
        assert_eq!(connector.channel(), "telegram");
        connector
            .handle_outbound(
                "evt-2",
                "telegram:chat:test",
                Some("msg-2".into()),
                Some("world".into()),
                vec![],
                serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(runtime.events.lock().unwrap().len(), 1);
    }
}
