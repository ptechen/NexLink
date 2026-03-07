use libp2p::PeerId;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct NodeScore {
    pub latency_ms: Option<u64>,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_seen: Instant,
    pub connected: bool,
    pub throughput_history: Vec<(Instant, f64)>, // Throughput over time
    pub latency_history: Vec<(Instant, u64)>,    // Latency history for trend analysis
    pub availability_score: f64,                 // Availability based on uptime
    pub reputation_score: f64,                   // Overall reputation score
    pub behavioral_trend: f64,                   // Trend in node behavior (positive/negative)
    pub ai_predictability_score: f64,            // How predictable the node's behavior is
    pub predicted_performance: f64,              // Predicted future performance
}

impl NodeScore {
    pub fn new() -> Self {
        Self {
            latency_ms: None,
            success_count: 0,
            failure_count: 0,
            last_seen: Instant::now(),
            connected: false,
            throughput_history: Vec::new(),
            latency_history: Vec::new(),
            availability_score: 0.0,
            reputation_score: 50.0, // Neutral starting point
            behavioral_trend: 0.0,
            ai_predictability_score: 0.0,
            predicted_performance: 0.0,
        }
    }

    /// Calculates a comprehensive score based on all factors
    pub fn score(&self) -> f64 {
        let latency_score = match self.latency_ms {
            Some(ms) => 1000.0 / (ms as f64 + 1.0),
            None => 0.0,
        };

        let success_rate = self.success_rate();
        let success_score = success_rate * 100.0;

        let availability_bonus = self.availability_score * 0.5; // 0-50 bonus
        let trend_factor = (self.behavioral_trend + 1.0) * 0.5; // Normalize from [-1, 1] to [0, 1]

        // Combine all factors with weights
        let base_score = latency_score + success_score;
        let trend_adjustment = base_score * trend_factor * 0.3; // Up to 30% adjustment based on trend

        let total_score = base_score + availability_bonus + trend_adjustment;

        // Apply a small penalty for stale information
        let stale_penalty = if self.last_seen.elapsed().as_secs() > 300 { // 5 minutes
            25.0
        } else {
            0.0
        };

        total_score - stale_penalty
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total > 0 {
            self.success_count as f64 / total as f64
        } else {
            0.5 // Neutral rate when no data
        }
    }

    /// Updates the AI prediction and behavioral metrics
    pub fn update_behavioral_metrics(&mut self) {
        // Update availability score based on connection history
        let uptime_percentage = if self.connected { 100.0 } else { 0.0 }; // Simplified
        self.availability_score = uptime_percentage;

        // Update behavioral trend based on recent activity
        self.update_behavioral_trend();

        // Calculate predictability based on consistency of performance
        self.update_predictability_score();

        // Predict future performance based on historical data
        self.update_predicted_performance();
    }

    /// Calculate trend in node behavior
    fn update_behavioral_trend(&mut self) {
        // For simplicity, we calculate trend based on recent success/failure ratio
        // In a full implementation, this would use more sophisticated ML models
        let recent_successes = self.success_count.saturating_sub(self.failure_count);
        let total_recent = self.success_count + self.failure_count;

        if total_recent > 0 {
            // Simple trend calculation: positive if more successes, negative if more failures
            self.behavioral_trend = if recent_successes > 0 {
                (recent_successes as f64 / total_recent as f64).min(1.0)
            } else {
                -(self.failure_count as f64 / total_recent as f64).max(-1.0)
            };
        }
    }

    /// Calculate how predictable the node's behavior is
    fn update_predictability_score(&mut self) {
        // Calculate how consistent the node's performance is
        if self.latency_history.len() < 2 {
            self.ai_predictability_score = 0.0;
            return;
        }

        // Calculate coefficient of variation for latency (lower CV = more predictable)
        let latencies: Vec<f64> = self.latency_history.iter()
            .take(10) // Look at last 10 measurements
            .map(|(_, lat)| *lat as f64)
            .collect();

        if latencies.is_empty() {
            self.ai_predictability_score = 0.0;
            return;
        }

        let mean_latency: f64 = latencies.iter().sum::<f64>() / latencies.len() as f64;
        if mean_latency == 0.0 {
            self.ai_predictability_score = 0.0;
            return;
        }

        let variance: f64 = latencies.iter()
            .map(|&x| (x - mean_latency).powi(2))
            .sum::<f64>() / latencies.len() as f64;

        let std_dev = variance.sqrt();
        let coefficient_of_variation = std_dev / mean_latency;

        // Lower coefficient of variation means higher predictability
        // Map from [0, infinity) to [0, 1] scale where 0 = unpredictable, 1 = perfectly predictable
        self.ai_predictability_score = 1.0 / (1.0 + coefficient_of_variation);
    }

    /// Predict future performance based on historical data
    fn update_predicted_performance(&mut self) {
        // This is a simplified prediction model
        // In a full implementation, this would use ML algorithms

        let current_score = self.score();

        // Factor in trend and predictability
        let trend_weight = 0.3;
        let predictability_weight = 0.2;
        let base_performance_weight = 0.5;

        self.predicted_performance =
            base_performance_weight * current_score +
            trend_weight * (self.behavioral_trend * 100.0) +  // Scale trend to [0, 100] range
            predictability_weight * (self.ai_predictability_score * 100.0);  // Scale predictability to [0, 100] range
    }

    /// Record a new latency measurement
    pub fn record_latency(&mut self, rtt_ms: u64) {
        self.latency_ms = Some(rtt_ms);
        self.last_seen = Instant::now();

        // Store in history for trend analysis
        self.latency_history.push((Instant::now(), rtt_ms));
        if self.latency_history.len() > 20 { // Keep only last 20 measurements
            self.latency_history.remove(0);
        }
    }

    /// Record a throughput measurement
    pub fn record_throughput(&mut self, bps: f64) {
        self.throughput_history.push((Instant::now(), bps));
        if self.throughput_history.len() > 20 { // Keep only last 20 measurements
            self.throughput_history.remove(0);
        }
    }
}

#[derive(Debug, Clone)]
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
        entry.record_latency(rtt_ms);
        entry.update_behavioral_metrics(); // Update all behavioral metrics
    }

    pub fn record_success(&mut self, peer: PeerId) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.success_count += 1;
        entry.last_seen = Instant::now();
        entry.update_behavioral_metrics(); // Update all behavioral metrics
    }

    pub fn record_failure(&mut self, peer: PeerId) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.failure_count += 1;
        entry.update_behavioral_metrics(); // Update all behavioral metrics
    }

    pub fn set_connected(&mut self, peer: PeerId, connected: bool) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.connected = connected;
        entry.last_seen = Instant::now();
        entry.update_behavioral_metrics(); // Update all behavioral metrics
    }

    pub fn remove_peer(&mut self, peer: &PeerId) {
        self.scores.remove(peer);
        if self.current.as_ref() == Some(peer) {
            self.current = None;
        }
    }

    /// Select the best connected node using AI-driven selection
    /// This method considers not just current performance but also predictability, trends, and other factors
    pub fn select_best(&mut self) -> Option<PeerId> {
        let best = self
            .scores
            .iter()
            .filter(|(_, s)| s.connected)
            .max_by(|(_, a), (_, b)| {
                // Use a more sophisticated scoring that includes AI predictions
                let score_a = self.ai_enhanced_score(a);
                let score_b = self.ai_enhanced_score(b);

                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(p, _)| *p);

        if best != self.current {
            self.current = best;
            best
        } else {
            None
        }
    }

    /// Enhanced scoring algorithm that incorporates AI predictions and behavioral analysis
    fn ai_enhanced_score(&self, node_score: &NodeScore) -> f64 {
        // Base score from the original algorithm
        let base_score = node_score.score();

        // AI-predicted performance factor (0-100 scale)
        let predicted_performance_factor = node_score.predicted_performance / 100.0;

        // Predictability factor (0-1 scale)
        let predictability_factor = node_score.ai_predictability_score;

        // Behavioral trend factor (-1 to 1 scale, normalized to 0-1)
        let trend_factor = (node_score.behavioral_trend + 1.0) / 2.0;

        // Weighted combination
        let ai_weight = 0.3;  // How much weight to give AI predictions

        base_score * (1.0 - ai_weight) +
        (predicted_performance_factor * 100.0 + predictability_factor * 50.0 + trend_factor * 50.0) * ai_weight
    }

    pub fn current(&self) -> Option<PeerId> {
        self.current
    }

    pub fn set_current(&mut self, peer: Option<PeerId>) {
        self.current = peer;
    }

    /// Get peer info for frontend display: (peer_id, latency_ms, is_selected, ai_score)
    pub fn peer_scores(&self) -> Vec<(PeerId, Option<u64>, bool, f64)> {
        self.scores
            .iter()
            .filter(|(_, s)| s.connected)
            .map(|(p, s)| (*p, s.latency_ms, self.current == Some(*p), self.ai_enhanced_score(s)))
            .collect()
    }

    /// Get all connected peers with detailed scoring information
    pub fn detailed_peer_scores(&self) -> Vec<(PeerId, NodeScore)> {
        self.scores
            .iter()
            .filter(|(_, s)| s.connected)
            .map(|(p, s)| (*p, s.clone()))
            .collect()
    }

    /// Get the AI prediction for a specific peer
    pub fn get_prediction(&self, peer: &PeerId) -> Option<f64> {
        self.scores.get(peer).map(|s| s.predicted_performance)
    }

    /// Record throughput for a peer
    pub fn record_throughput(&mut self, peer: PeerId, bps: f64) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.record_throughput(bps);
        entry.update_behavioral_metrics(); // Update all behavioral metrics
    }

    /// Get availability statistics for all peers
    pub fn get_availability_stats(&self) -> HashMap<PeerId, f64> {
        self.scores
            .iter()
            .map(|(peer, score)| (*peer, score.availability_score))
            .collect()
    }
}
