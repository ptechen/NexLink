use crate::network::autonomous::AutonomousNetworkManager;
use crate::node_score::NodeSelector;
use crate::traffic::TrafficCounter;
use libp2p::PeerId;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

/// Integration layer between autonomous network manager and existing network stack
pub struct AutonomousNetworkIntegration {
    pub manager: Arc<AutonomousNetworkManager>,
    pub node_selector: Arc<RwLock<NodeSelector>>,
    pub traffic_counter: Arc<TrafficCounter>,
}

impl AutonomousNetworkIntegration {
    pub fn new(
        manager: Arc<AutonomousNetworkManager>,
        node_selector: Arc<RwLock<NodeSelector>>,
        traffic_counter: Arc<TrafficCounter>,
    ) -> Self {
        Self {
            manager,
            node_selector,
            traffic_counter,
        }
    }

    /// Start the autonomous network monitoring and decision-making system
    pub async fn start_monitoring(&self) {
        let manager = self.manager.clone();
        let node_selector = self.node_selector.clone();
        let traffic_counter = self.traffic_counter.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10)); // Run every 10 seconds

            loop {
                interval.tick().await;

                // Update network metrics
                Self::update_network_metrics(&manager, &node_selector, &traffic_counter).await;

                // Make autonomous decisions based on current network state
                Self::make_autonomous_decisions(&manager, &node_selector, &traffic_counter).await;

                // Check network health and provide recommendations
                Self::check_network_health(&manager).await;
            }
        });
    }

    /// Update network metrics that feed into autonomous decision making
    async fn update_network_metrics(
        manager: &Arc<AutonomousNetworkManager>,
        node_selector: &Arc<RwLock<NodeSelector>>,
        traffic_counter: &Arc<TrafficCounter>,
    ) {
        // Get node scores from the selector
        let detailed_scores = {
            let selector = node_selector.read().await;
            selector.detailed_peer_scores()
        };

        // Update node scores in the autonomous manager
        for (peer_id, node_score) in detailed_scores {
            let score = node_score.score();
            manager.update_node_score(peer_id, score).await;
        }

        // Get traffic predictions and update autonomous system
        if let Some(prediction) = traffic_counter.get_current_prediction() {
            let pattern_key = "bandwidth_prediction".to_string();
            let metrics = vec![
                prediction.predicted_upload_bps,
                prediction.predicted_download_bps,
                prediction.confidence,
            ];
            manager.analyze_traffic_pattern(pattern_key, metrics).await;
        }

        // Update network efficiency metric
        {
            let mut behaviors = manager.behaviors.write().await;
            let current_prediction = traffic_counter.get_current_prediction();

            if let Some(pred) = current_prediction {
                // Use prediction confidence as a factor in network efficiency
                behaviors.update_network_efficiency(pred.confidence);
            }
        }
    }

    /// Make autonomous decisions based on network metrics
    async fn make_autonomous_decisions(
        manager: &Arc<AutonomousNetworkManager>,
        node_selector: &Arc<RwLock<NodeSelector>>,
        traffic_counter: &Arc<TrafficCounter>,
    ) {
        // Check for traffic anomalies
        let anomalies = traffic_counter.get_anomaly_report();

        if !anomalies.is_empty() {
            info!("Detected {} traffic anomalies", anomalies.len());

            // Example decision: trigger load balancing when anomalies are detected
            let decision = crate::network::behaviour::AiDecision::LoadBalancing {
                target_nodes: Vec::new(), // Will be populated based on available nodes
            };

            manager.make_autonomous_decision(decision, 0.8).await;
        }

        // Check node performance and make decisions about routing
        let scores: Vec<_> = {
            let selector = node_selector.read().await;
            selector.peer_scores()
        };

        // Identify underperforming nodes
        let poor_performers: Vec<_> = scores
            .iter()
            .filter(|(_, _, _, ai_score)| *ai_score < 30.0) // Low AI-enhanced score
            .map(|(peer_id, _, _, _)| peer_id.to_string())
            .collect();

        if !poor_performers.is_empty() {
            info!("Identified {} poor performing nodes", poor_performers.len());

            // Example decision: reroute traffic from poor performers
            let decision = crate::network::behaviour::AiDecision::RouteOptimization {
                path: Vec::new(), // Would be calculated in a real implementation
                efficiency_score: 0.5,
            };

            manager.make_autonomous_decision(decision, 0.7).await;
        }

        // Check traffic prediction and make capacity decisions
        if let Some(prediction) = traffic_counter.get_current_prediction() {
            // If predicted traffic is high and confidence is high, prepare for scaling
            if prediction.confidence > 0.7 &&
               (prediction.predicted_upload_bps > 1_000_000.0 || // 1 Mbps threshold
                prediction.predicted_download_bps > 1_000_000.0) {

                info!(
                    "High traffic predicted: upload={} bps, download={} bps",
                    prediction.predicted_upload_bps,
                    prediction.predicted_download_bps
                );

                // Example decision: allocate more resources for predicted high traffic
                let decision = crate::network::behaviour::AiDecision::ResourceAllocation {
                    node_id: "gateway".to_string(),
                    resources: 100, // Would be more sophisticated in practice
                };

                manager.make_autonomous_decision(decision, 0.9).await;
            }
        }

        // Use the new reputation-based anomaly detection
        let reputation_anomalies = manager.detect_node_anomalies().await;
        if !reputation_anomalies.is_empty() {
            info!("Detected {} reputation-based anomalies", reputation_anomalies.len());

            for (peer_id, anomaly_type) in reputation_anomalies {
                if anomaly_type == "performance_degradation" {
                    // Take action on degrading nodes
                    info!("Node {} showing performance degradation", peer_id.to_string());

                    // Make a decision to reduce reliance on this node
                    let decision = crate::network::behaviour::AiDecision::RouteOptimization {
                        path: vec![peer_id.to_string()], // Avoid this node
                        efficiency_score: 0.3,
                    };

                    manager.make_autonomous_decision(decision, 0.75).await;
                }
            }
        }

        // Use the new bandwidth prediction method
        if let Some(prediction) = traffic_counter.get_current_prediction() {
            manager.make_bandwidth_based_decision(
                prediction.predicted_upload_bps,
                prediction.predicted_download_bps
            ).await;
        }

        // Security-related decisions
        Self::make_security_decisions(manager, node_selector).await;
    }

    /// Make security-related autonomous decisions
    async fn make_security_decisions(
        manager: &Arc<AutonomousNetworkManager>,
        node_selector: &Arc<RwLock<NodeSelector>>,
    ) {
        // Get all nodes
        let all_nodes = {
            let selector = node_selector.read().await;
            selector.detailed_peer_scores() // 使用实际存在的方法
        };

        // Assess potential security threats
        for (peer_id, _) in all_nodes {
            // Calculate reputation score for the node
            let reputation = manager.calculate_reputation_score(peer_id).await;

            // If reputation is very low, consider it a security risk
            if reputation < 0.3 {
                info!("Low reputation detected for node {}, considering security alert", peer_id);

                // Create a security alert decision
                let decision = crate::network::behaviour::AiDecision::SecurityAlert {
                    threat_level: 3, // High threat level
                    affected_resources: vec![peer_id.to_string()],
                };

                manager.make_autonomous_decision(decision, 0.85).await;
            }
        }
    }

    /// Check overall network health and log recommendations
    async fn check_network_health(manager: &Arc<AutonomousNetworkManager>) {
        let health_score = manager.get_network_health_score().await;
        let recommendations = manager.get_recommendations().await;

        info!("Network health score: {:.2}", health_score);

        if health_score < 0.5 {
            warn!("Network health is below optimal levels");
        }

        if !recommendations.is_empty() {
            info!("Autonomous recommendations:");
            for recommendation in recommendations {
                info!("  - {}", recommendation);
            }
        }
    }

    /// Update a specific node's score in the autonomous system
    pub async fn update_node_performance(&self, peer_id: PeerId, performance_metric: f64) {
        self.manager.update_node_score(peer_id, performance_metric).await;
    }

    /// Get current autonomous network recommendations
    pub async fn get_recommendations(&self) -> Vec<String> {
        self.manager.get_recommendations().await
    }

    /// Get current network health score
    pub async fn get_health_score(&self) -> f64 {
        self.manager.get_network_health_score().await
    }
}