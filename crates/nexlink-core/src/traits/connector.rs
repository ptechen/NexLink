use async_trait::async_trait;

use crate::{MessageOutboundPayload, OutboundMessageContext};

#[async_trait]
pub trait Connector: Send + Sync {
    fn channel(&self) -> &'static str;

    async fn start(&self) -> anyhow::Result<()>;

    async fn send_message(
        &self,
        session_key: &str,
        payload: MessageOutboundPayload,
    ) -> anyhow::Result<()>;

    async fn send_context(&self, msg: OutboundMessageContext) -> anyhow::Result<()> {
        self.send_message(&msg.session_key, msg.payload).await
    }
}
