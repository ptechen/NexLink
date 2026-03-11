use std::sync::{Arc, LazyLock};
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use libp2p::PeerId;
use copy_bidirectional::copy_bidirectional::TrafficTrait;

pub static NEXLINK_TRAFFIC: LazyLock<DashMap<PeerId, Traffic>> = LazyLock::new(DashMap::new);
#[derive(Clone)]
pub struct ProviderTrafficCounter {
    pub peer_id: PeerId,
}

impl TrafficTrait for ProviderTrafficCounter {
    fn add(info: &Arc<Self>, size: u64, is_upload: bool) {
        if is_upload {
            add_upload(info.peer_id, size);
        } else {
            add_download(info.peer_id, size);
        }
    }
}

#[derive(Debug, Default)]
pub struct Traffic {
    pub upload: AtomicU64,
    pub download: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct TrafficSnapshot {
    pub peer_id: PeerId,
    pub upload: u64,
    pub download: u64,
}

pub fn add_upload(peer_id: PeerId, size: u64) {
    let traffic = NEXLINK_TRAFFIC.entry(peer_id).or_default();
    traffic.upload.fetch_add(size, Ordering::Relaxed);
}

pub fn add_download(peer_id: PeerId, size: u64) {
    let traffic = NEXLINK_TRAFFIC.entry(peer_id).or_default();
    traffic.download.fetch_add(size, Ordering::Relaxed);
}

pub fn snapshot(peer_id: PeerId) -> TrafficSnapshot {
    let traffic = NEXLINK_TRAFFIC.entry(peer_id).or_default();
    TrafficSnapshot {
        peer_id,
        upload: traffic.upload.load(Ordering::Relaxed),
        download: traffic.download.load(Ordering::Relaxed),
    }
}

pub fn snapshot_all() -> Vec<TrafficSnapshot> {
    let mut snapshots: Vec<_> = NEXLINK_TRAFFIC
        .iter()
        .map(|entry| TrafficSnapshot {
            peer_id: *entry.key(),
            upload: entry.upload.load(Ordering::Relaxed),
            download: entry.download.load(Ordering::Relaxed),
        })
        .collect();
    snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.upload + snapshot.download));
    snapshots
}
