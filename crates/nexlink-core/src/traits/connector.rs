use async_trait::async_trait;

use crate::{MessageOutboundPayload, OutboundConnectorMessage, OutboundMessageContext};

#[async_trait]
pub trait Connector: Send + Sync {
    fn channel(&self) -> &str;

    async fn start(&self) -> anyhow::Result<()>;

    async fn send_message(
        &self,
        session_key: &str,
        payload: MessageOutboundPayload,
    ) -> anyhow::Result<()>;

    async fn send_context(&self, msg: OutboundMessageContext) -> anyhow::Result<()> {
        self.send_message(&msg.session_key, msg.payload).await
    }

    async fn send_connector(&self, msg: OutboundConnectorMessage) -> anyhow::Result<()> {
        self.send_message(
            &msg.session_key,
            MessageOutboundPayload {
                reply_to: msg.reply_to,
                text: msg.text,
                attachments: msg.attachments,
                metadata: msg.metadata,
            },
        )
        .await
    }
}
