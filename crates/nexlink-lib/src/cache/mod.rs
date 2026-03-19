use std::sync::LazyLock;

use dashmap::{DashMap, DashSet};
use libp2p::PeerId;

/// 当前连接到 relay 的所有 peer
pub static CONNECTED_PEERS: LazyLock<DashSet<PeerId>> = LazyLock::new(DashSet::new);

/// 已通过 rendezvous 注册的 provider peer
pub static PROVIDER_PEERS: LazyLock<DashSet<PeerId>> = LazyLock::new(DashSet::new);

/// PeerId → 数据库 peer_user.id 缓存
pub static PEER_CACHE: LazyLock<DashMap<PeerId, i64>> = LazyLock::new(DashMap::new);
