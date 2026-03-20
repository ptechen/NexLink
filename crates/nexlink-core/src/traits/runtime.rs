use async_trait::async_trait;

use crate::{EventEnvelope, InvokeRequest, InvokeResponse};

#[async_trait]
pub trait Runtime: Send + Sync {
    async fn handle_event(&self, event: EventEnvelope) -> anyhow::Result<()>;

    async fn invoke_capability(&self, request: InvokeRequest) -> anyhow::Result<InvokeResponse>;
}
