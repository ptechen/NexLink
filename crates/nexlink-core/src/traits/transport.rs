use async_trait::async_trait;

use crate::{EventEnvelope, Peer};

#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, target_peer: &str, envelope: EventEnvelope) -> anyhow::Result<()>;

    async fn publish_local(&self, envelope: EventEnvelope) -> anyhow::Result<()>;

    async fn resolve_peer(&self, peer_id: &str) -> anyhow::Result<Option<Peer>>;
}
