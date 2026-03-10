use std::sync::atomic::AtomicU64;
use std::sync::LazyLock;
use dashmap::DashMap;
use libp2p::PeerId;

pub static NEXLINK_TRAFFIC:LazyLock<DashMap<PeerId, Traffic>> = LazyLock::new(|| DashMap::new());

pub struct Traffic {
    pub upload: AtomicU64,
    pub download: AtomicU64,
}