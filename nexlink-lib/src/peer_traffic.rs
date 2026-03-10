use std::io;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerTrafficRule {
    pub byte_quota: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerTrafficSnapshot {
    pub peer_id: String,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: u32,
    pub total_connections: u32,
    pub quota_bytes: Option<u64>,
    pub quota_exceeded: bool,
    pub last_seen_unix_secs: u64,
}

#[derive(Debug, Clone, Default)]
pub struct PeerTrafficManager {
    peers: Arc<DashMap<String, Arc<PeerTrafficEntry>>>,
}

#[derive(Debug)]
struct PeerTrafficEntry {
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    active_connections: AtomicU32,
    total_connections: AtomicU32,
    quota_bytes: AtomicU64,
    last_seen_unix_secs: AtomicU64,
}

impl Default for PeerTrafficEntry {
    fn default() -> Self {
        Self {
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            active_connections: AtomicU32::new(0),
            total_connections: AtomicU32::new(0),
            quota_bytes: AtomicU64::new(0),
            last_seen_unix_secs: AtomicU64::new(now_unix_secs()),
        }
    }
}

impl PeerTrafficManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_rule(&self, peer_id: PeerId, rule: PeerTrafficRule) {
        let entry = self.entry_for(peer_id);
        entry.set_quota(rule.byte_quota);
        entry.touch();
    }

    pub fn can_open_connection(&self, peer_id: PeerId) -> Result<()> {
        let entry = self.entry_for(peer_id);
        if entry.quota_exceeded() {
            return Err(anyhow!("peer {peer_id} exceeded configured traffic quota"));
        }
        Ok(())
    }

    pub fn open_connection(&self, peer_id: PeerId) -> Result<()> {
        self.can_open_connection(peer_id)?;
        let entry = self.entry_for(peer_id);
        entry.active_connections.fetch_add(1, Ordering::Relaxed);
        entry.total_connections.fetch_add(1, Ordering::Relaxed);
        entry.touch();
        Ok(())
    }

    pub fn close_connection(&self, peer_id: PeerId) {
        let entry = self.entry_for(peer_id);
        entry.active_connections.fetch_sub(1, Ordering::Relaxed);
        entry.touch();
    }

    pub fn record_sent(&self, peer_id: PeerId, n: u64) -> io::Result<()> {
        let entry = self.entry_for(peer_id);
        entry.bytes_sent.fetch_add(n, Ordering::Relaxed);
        entry.touch();
        if entry.quota_exceeded() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("peer {peer_id} exceeded configured traffic quota"),
            ));
        }
        Ok(())
    }

    pub fn record_received(&self, peer_id: PeerId, n: u64) -> io::Result<()> {
        let entry = self.entry_for(peer_id);
        entry.bytes_received.fetch_add(n, Ordering::Relaxed);
        entry.touch();
        if entry.quota_exceeded() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("peer {peer_id} exceeded configured traffic quota"),
            ));
        }
        Ok(())
    }

    pub fn snapshot(&self, peer_id: PeerId) -> PeerTrafficSnapshot {
        self.entry_for(peer_id).snapshot(peer_id.to_string())
    }

    pub fn snapshot_all(&self) -> Vec<PeerTrafficSnapshot> {
        let mut snapshots: Vec<_> = self
            .peers
            .iter()
            .map(|entry| entry.value().snapshot(entry.key().clone()))
            .collect();
        snapshots.sort_by_key(|s| std::cmp::Reverse(s.bytes_sent + s.bytes_received));
        snapshots
    }
}

impl PeerTrafficEntry {
    fn set_quota(&self, quota: Option<u64>) {
        self.quota_bytes.store(quota.unwrap_or(0), Ordering::Relaxed);
    }

    fn quota(&self) -> Option<u64> {
        match self.quota_bytes.load(Ordering::Relaxed) {
            0 => None,
            n => Some(n),
        }
    }

    fn total_bytes(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed) + self.bytes_received.load(Ordering::Relaxed)
    }

    fn quota_exceeded(&self) -> bool {
        match self.quota() {
            Some(limit) => self.total_bytes() >= limit,
            None => false,
        }
    }

    fn touch(&self) {
        self.last_seen_unix_secs
            .store(now_unix_secs(), Ordering::Relaxed);
    }

    fn snapshot(&self, peer_id: String) -> PeerTrafficSnapshot {
        PeerTrafficSnapshot {
            peer_id,
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            quota_bytes: self.quota(),
            quota_exceeded: self.quota_exceeded(),
            last_seen_unix_secs: self.last_seen_unix_secs.load(Ordering::Relaxed),
        }
    }
}

impl PeerTrafficManager {
    fn entry_for(&self, peer_id: PeerId) -> Arc<PeerTrafficEntry> {
        let key = peer_id.to_string();
        self.peers
            .entry(key)
            .or_insert_with(|| Arc::new(PeerTrafficEntry::default()))
            .clone()
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_traffic_per_peer() {
        let manager = PeerTrafficManager::new();
        let peer = PeerId::random();
        manager.open_connection(peer).unwrap();
        manager.record_received(peer, 10).unwrap();
        manager.record_sent(peer, 20).unwrap();
        manager.close_connection(peer);

        let snap = manager.snapshot(peer);
        assert_eq!(snap.bytes_received, 10);
        assert_eq!(snap.bytes_sent, 20);
        assert_eq!(snap.active_connections, 0);
        assert_eq!(snap.total_connections, 1);
    }

    #[test]
    fn enforces_quota() {
        let manager = PeerTrafficManager::new();
        let peer = PeerId::random();
        manager.set_rule(
            peer,
            PeerTrafficRule {
                byte_quota: Some(16),
            },
        );
        manager.open_connection(peer).unwrap();
        manager.record_received(peer, 8).unwrap();
        manager.record_sent(peer, 8).unwrap_err();
    }
}
