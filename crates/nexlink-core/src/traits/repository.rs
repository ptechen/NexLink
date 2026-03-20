use async_trait::async_trait;

use crate::{Bot, Capability, EventEnvelope, Peer, Policy, Session};

#[async_trait]
pub trait PeerRepository: Send + Sync {
    async fn upsert(&self, peer: Peer) -> anyhow::Result<()>;
    async fn get(&self, peer_id: &str) -> anyhow::Result<Option<Peer>>;
}

#[async_trait]
pub trait BotRepository: Send + Sync {
    async fn upsert(&self, bot: Bot) -> anyhow::Result<()>;
    async fn get(&self, bot_id: &str) -> anyhow::Result<Option<Bot>>;
}

#[async_trait]
pub trait CapabilityRepository: Send + Sync {
    async fn register(&self, capability: Capability) -> anyhow::Result<()>;
    async fn list_by_name(&self, name: &str) -> anyhow::Result<Vec<Capability>>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn upsert(&self, session: Session) -> anyhow::Result<()>;
    async fn get(&self, session_key: &str) -> anyhow::Result<Option<Session>>;
}

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn append(&self, event: EventEnvelope) -> anyhow::Result<()>;
}

#[async_trait]
pub trait PolicyRepository: Send + Sync {
    async fn upsert(&self, policy: Policy) -> anyhow::Result<()>;
    async fn get(&self, policy_id: &str) -> anyhow::Result<Option<Policy>>;
}
