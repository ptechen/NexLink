use libp2p::PeerId;
use std::collections::HashMap;
use std::time::Instant;

pub struct NodeScore {
    pub latency_ms: Option<u64>,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_seen: Instant,
    pub connected: bool,
}

impl NodeScore {
    pub fn new() -> Self {
        Self {
            latency_ms: None,
            success_count: 0,
            failure_count: 0,
            last_seen: Instant::now(),
            connected: false,
        }
    }

    pub fn score(&self) -> f64 {
        let latency_score = match self.latency_ms {
            Some(ms) => 1000.0 / (ms as f64 + 1.0),
            None => 0.0,
        };
        let total = self.success_count + self.failure_count;
        let success_ratio = if total > 0 {
            self.success_count as f64 / total as f64
        } else {
            0.5
        };
        let stale_penalty = if self.last_seen.elapsed().as_secs() > 60 {
            50.0
        } else {
            0.0
        };
        latency_score + success_ratio * 100.0 - stale_penalty
    }
}

pub struct NodeSelector {
    scores: HashMap<PeerId, NodeScore>,
    current: Option<PeerId>,
}

impl NodeSelector {
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
            current: None,
        }
    }

    pub fn update_latency(&mut self, peer: PeerId, rtt_ms: u64) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.latency_ms = Some(rtt_ms);
        entry.last_seen = Instant::now();
    }

    pub fn record_success(&mut self, peer: PeerId) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.success_count += 1;
        entry.last_seen = Instant::now();
    }

    pub fn record_failure(&mut self, peer: PeerId) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.failure_count += 1;
    }

    pub fn set_connected(&mut self, peer: PeerId, connected: bool) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.connected = connected;
        entry.last_seen = Instant::now();
    }

    pub fn remove_peer(&mut self, peer: &PeerId) {
        self.scores.remove(peer);
        if self.current.as_ref() == Some(peer) {
            self.current = None;
        }
    }

    /// Select the best connected node. Returns Some(peer) if selection changed, None if unchanged.
    pub fn select_best(&mut self) -> Option<PeerId> {
        let best = self
            .scores
            .iter()
            .filter(|(_, s)| s.connected)
            .max_by(|(_, a), (_, b)| {
                a.score()
                    .partial_cmp(&b.score())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(p, _)| *p);

        if best != self.current {
            self.current = best;
            best
        } else {
            None
        }
    }

    pub fn current(&self) -> Option<PeerId> {
        self.current
    }

    pub fn set_current(&mut self, peer: Option<PeerId>) {
        self.current = peer;
    }

    /// Get peer info for frontend display: (peer_id, latency_ms, is_selected)
    pub fn peer_scores(&self) -> Vec<(PeerId, Option<u64>, bool)> {
        self.scores
            .iter()
            .filter(|(_, s)| s.connected)
            .map(|(p, s)| (*p, s.latency_ms, self.current == Some(*p)))
            .collect()
    }
}
