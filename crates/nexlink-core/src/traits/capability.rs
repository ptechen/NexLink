use async_trait::async_trait;

use crate::{Capability, InvokeRequest, InvokeResponse};

#[async_trait]
pub trait CapabilityProvider: Send + Sync {
    fn descriptor(&self) -> Capability;

    async fn invoke(&self, request: InvokeRequest) -> anyhow::Result<InvokeResponse>;
}
