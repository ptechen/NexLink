use std::sync::{Arc, LazyLock};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use copy_bidirectional::copy_bidirectional::TrafficTrait;
use dashmap::DashMap;
use libp2p::PeerId;

pub static NEXLINK_TRAFFIC: LazyLock<DashMap<PeerId, Traffic>> = LazyLock::new(DashMap::new);
pub static NEXLINK_TRAFFIC_CONTEXT: LazyLock<DashMap<PeerId, TrafficContext>> =
    LazyLock::new(DashMap::new);

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
    pub active_connections: AtomicU32,
}

#[derive(Debug, Clone, Default)]
pub struct TrafficContext {
    pub role: Option<String>,
    pub source: Option<String>,
    pub source_ip: Option<String>,
    pub source_transport: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TrafficSnapshot {
    pub peer_id: PeerId,
    pub upload: u64,
    pub download: u64,
    pub active_connections: u32,
    pub role: Option<String>,
    pub source: Option<String>,
    pub source_ip: Option<String>,
    pub source_transport: Option<String>,
}

pub fn add_upload(peer_id: PeerId, size: u64) {
    let traffic = NEXLINK_TRAFFIC.entry(peer_id).or_default();
    traffic.upload.fetch_add(size, Ordering::Relaxed);
}

pub fn add_download(peer_id: PeerId, size: u64) {
    let traffic = NEXLINK_TRAFFIC.entry(peer_id).or_default();
    traffic.download.fetch_add(size, Ordering::Relaxed);
}

pub fn inc_active_connections(peer_id: PeerId) {
    let traffic = NEXLINK_TRAFFIC.entry(peer_id).or_default();
    traffic.active_connections.fetch_add(1, Ordering::Relaxed);
}

pub fn dec_active_connections(peer_id: PeerId) {
    let traffic = NEXLINK_TRAFFIC.entry(peer_id).or_default();
    traffic.active_connections.fetch_sub(1, Ordering::Relaxed);
}

pub fn update_context(peer_id: PeerId, update: TrafficContextUpdate<'_>) {
    let mut context = NEXLINK_TRAFFIC_CONTEXT
        .entry(peer_id)
        .or_default();

    if let Some(role) = update.role {
        context.role = Some(role.to_string());
    }
    if let Some(source) = update.source {
        context.source = Some(source.to_string());
    }
    if let Some(source_ip) = update.source_ip {
        context.source_ip = Some(source_ip.to_string());
    }
    if let Some(source_transport) = update.source_transport {
        context.source_transport = Some(source_transport.to_string());
    }
}

pub fn snapshot_all() -> Vec<TrafficSnapshot> {
    NEXLINK_TRAFFIC
        .iter()
        .map(|entry| {
            let peer_id = *entry.key();
            let context = NEXLINK_TRAFFIC_CONTEXT.get(&peer_id).map(|ctx| ctx.clone());
            TrafficSnapshot {
                peer_id,
                upload: entry.upload.load(Ordering::Relaxed),
                download: entry.download.load(Ordering::Relaxed),
                active_connections: entry.active_connections.load(Ordering::Relaxed),
                role: context.as_ref().and_then(|ctx| ctx.role.clone()),
                source: context.as_ref().and_then(|ctx| ctx.source.clone()),
                source_ip: context.as_ref().and_then(|ctx| ctx.source_ip.clone()),
                source_transport: context.as_ref().and_then(|ctx| ctx.source_transport.clone()),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct TrafficContextUpdate<'a> {
    pub role: Option<&'a str>,
    pub source: Option<&'a str>,
    pub source_ip: Option<&'a str>,
    pub source_transport: Option<&'a str>,
}
