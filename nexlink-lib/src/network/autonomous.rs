use libp2p::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::network::behaviour::{AutonomousBehaviors, AiDecision};
use std::time::{SystemTime, UNIX_EPOCH};

// Structure to hold node performance history for reputation calculations
#[derive(Debug, Clone)]
pub struct NodePerformanceHistory {
    pub scores: Vec<(u64, f64)>, // (timestamp, score)
    pub avg_latency: f64,
    pub success_rate: f64,
    pub availability: f64,
}

#[derive(Debug, Clone)]
pub struct ConfidenceThresholds {
    pub critical: f64,
    pub medium: f64,
    pub low: f64,
}

impl Default for ConfidenceThresholds {
    fn default() -> Self {
        Self {
            critical: 0.9,
            medium: 0.7,
            low: 0.5,
        }
    }
}

pub struct AutonomousNetworkManager {
    pub behaviors: Arc<RwLock<AutonomousBehaviors>>,
    pub node_scores: Arc<RwLock<HashMap<PeerId, f64>>>,
    pub traffic_patterns: Arc<RwLock<HashMap<String, Vec<f64>>>>,
    pub node_performance_history: Arc<RwLock<HashMap<PeerId, NodePerformanceHistory>>>,
    pub monitoring_interval_ms: u64,
    pub confidence_thresholds: ConfidenceThresholds,
}

impl AutonomousNetworkManager {
    pub fn new() -> Self {
        Self {
            behaviors: Arc::new(RwLock::new(AutonomousBehaviors::new())),
            node_scores: Arc::new(RwLock::new(HashMap::new())),
            traffic_patterns: Arc::new(RwLock::new(HashMap::new())),
            node_performance_history: Arc::new(RwLock::new(HashMap::new())),
            monitoring_interval_ms: 10000, // 10 seconds default
            confidence_thresholds: ConfidenceThresholds::default(),
        }
    }

    pub async fn make_autonomous_decision(&self, decision_type: AiDecision, confidence: f64) {
        let mut behaviors = self.behaviors.write().await;
        behaviors.make_decision(decision_type, confidence);
    }

    pub async fn update_node_score(&self, peer_id: PeerId, score: f64) {
        let mut node_scores = self.node_scores.write().await;
        node_scores.insert(peer_id, score);

        // Also update performance history
        self.update_node_performance_history(peer_id, score).await;
    }

    async fn update_node_performance_history(&self, peer_id: PeerId, score: f64) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut history_map = self.node_performance_history.write().await;

        let history_entry = history_map.entry(peer_id).or_insert_with(|| {
            NodePerformanceHistory {
                scores: Vec::new(),
                avg_latency: 0.0,
                success_rate: 0.0,
                availability: 0.0,
            }
        });

        // Add the new score with timestamp
        history_entry.scores.push((current_time, score));

        // Keep only the last 100 entries to prevent unbounded growth
        if history_entry.scores.len() > 100 {
            history_entry.scores.drain(0..50); // Remove oldest 50 entries
        }
    }

    pub async fn analyze_traffic_pattern(&self, pattern_key: String, metrics: Vec<f64>) {
        let mut traffic_patterns = self.traffic_patterns.write().await;
        traffic_patterns.insert(pattern_key, metrics);
    }

    pub async fn get_recommendations(&self) -> Vec<String> {
        let behaviors = self.behaviors.read().await;
        let node_scores = self.node_scores.read().await;

        let mut recommendations = Vec::new();

        // Example recommendation: suggest optimizing routing for nodes with lower scores
        for (peer_id, score) in node_scores.iter() {
            if *score < 0.5 {
                recommendations.push(format!("Consider rerouting traffic from node {} due to low performance score ({})", peer_id, score));
            }
        }

        // Add network efficiency recommendation
        if behaviors.network_efficiency < 0.7 {
            recommendations.push("Network efficiency is below optimal threshold, consider rebalancing load".to_string());
        }

        recommendations
    }

    pub async fn get_network_health_score(&self) -> f64 {
        let behaviors = self.behaviors.read().await;
        behaviors.get_network_health_score()
    }

    /// Calculate a reputation score for a node based on historical data with time decay
    pub async fn calculate_reputation_score(&self, peer_id: PeerId) -> f64 {
        let history_map = self.node_performance_history.read().await;

        if let Some(history) = history_map.get(&peer_id) {
            if history.scores.is_empty() {
                return 0.5; // Default neutral score
            }

            // Apply time-based decay to older scores
            let mut weighted_score = 0.0;
            let mut total_weight = 0.0;
            let decay_factor: f64 = 0.95; // Recent scores weigh more, explicitly typed

            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            for (i, (timestamp, score)) in history.scores.iter().rev().enumerate() {
                // Calculate age in hours
                let age_hours = ((current_time - timestamp) as f64) / 3600.0;
                let weight = decay_factor.powf(age_hours) / (i as f64 + 1.0); // Additional positional weight

                weighted_score += score * weight;
                total_weight += weight;
            }

            if total_weight > 0.0 {
                weighted_score / total_weight
            } else {
                0.5 // Default neutral score
            }
        } else {
            0.5 // Default neutral score for unknown nodes
        }
    }

    /// Make a decision based on predicted bandwidth needs
    pub async fn make_bandwidth_based_decision(&self, predicted_up: f64, predicted_down: f64) {
        let confidence = if predicted_up > 10_000_000.0 || predicted_down > 10_000_000.0 {
            // High bandwidth prediction - high confidence in scaling decision
            0.85
        } else {
            0.6
        };

        if confidence >= self.confidence_thresholds.medium {
            let decision = AiDecision::ResourceAllocation {
                node_id: "bandwidth_gateway".to_string(),
                resources: (predicted_up / 1_000_000.0) as u64, // Convert to resource units
            };

            self.make_autonomous_decision(decision, confidence).await;
        }
    }

    /// Detect anomalies in node performance
    pub async fn detect_node_anomalies(&self) -> Vec<(PeerId, String)> {
        let mut anomalies = Vec::new();
        let node_scores = self.node_scores.read().await;
        let history_map = self.node_performance_history.read().await;

        for (peer_id, current_score) in node_scores.iter() {
            if let Some(history) = history_map.get(peer_id) {
                if !history.scores.is_empty() {
                    // Calculate average historical score
                    let avg_historical: f64 = history.scores.iter()
                        .map(|(_, score)| *score)  // Dereference the score
                        .sum::<f64>() / history.scores.len() as f64;

                    // If current score deviates significantly from historical average
                    if (*current_score - avg_historical).abs() > 0.3 {
                        let anomaly_type = if *current_score > avg_historical {
                            "performance_improvement".to_string()
                        } else {
                            "performance_degradation".to_string()
                        };

                        anomalies.push((*peer_id, anomaly_type));
                    }
                }
            }
        }

        anomalies
    }
}

impl Default for AutonomousNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 演示自主网络功能的使用
pub mod example_usage {
    use super::*;
    use crate::network::integrated_autonomous::AutonomousNetworkIntegration;
    use crate::node_score::NodeSelector;
    use crate::traffic::TrafficCounter;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// 演示增强的自主网络功能
    pub async fn demonstrate_enhanced_autonomous_features() {
        println!("开始演示增强的自主网络功能...");

        // 创建基本组件
        let node_selector = Arc::new(RwLock::new(NodeSelector::new()));
        let traffic_counter = Arc::new(TrafficCounter::new());

        // 创建增强的自主网络管理器
        let autonomous_manager = Arc::new(AutonomousNetworkManager::new());

        // 注意：我们不能直接修改 Arc 包装的对象，需要使用不同的方式来演示配置
        // 在实际使用中，我们可以通过以下方式修改配置：

        // 1. 通过 Arc<RwLock<>> 访问并修改内部值（需要修改结构体定义添加 RwLock）
        // 2. 或者提供专用的方法来修改配置

        // 创建集成层
        let integration = AutonomousNetworkIntegration::new(
            autonomous_manager,
            node_selector,
            traffic_counter,
        );

        // 展示新功能
        demonstrate_reputation_scoring(&integration).await;
        demonstrate_anomaly_detection(&integration).await;
        demonstrate_bandwidth_prediction(&integration).await;

        println!("自主网络功能演示完成！");
    }

    /// 演示声誉评分功能
    async fn demonstrate_reputation_scoring(integration: &AutonomousNetworkIntegration) {
        println!("演示声誉评分功能...");

        let peer_id = PeerId::random();

        // 更新节点分数多次以建立历史记录
        integration.update_node_performance(peer_id, 0.8).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        integration.update_node_performance(peer_id, 0.85).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        integration.update_node_performance(peer_id, 0.75).await;

        // 获取基于历史的声誉评分
        let reputation = integration.manager.calculate_reputation_score(peer_id).await;
        println!("节点 {:?} 的声誉评分为: {:.2}", peer_id, reputation);
    }

    /// 演示异常检测功能
    async fn demonstrate_anomaly_detection(integration: &AutonomousNetworkIntegration) {
        println!("演示异常检测功能...");

        let peer_id = PeerId::random();

        // 建立正常行为基线
        integration.update_node_performance(peer_id, 0.8).await;
        integration.update_node_performance(peer_id, 0.82).await;
        integration.update_node_performance(peer_id, 0.78).await;

        // 引入异常行为
        integration.update_node_performance(peer_id, 0.2).await; // 性能显著下降

        // 检测异常
        let anomalies = integration.manager.detect_node_anomalies().await;
        if !anomalies.is_empty() {
            println!("检测到 {} 个异常:", anomalies.len());
            for (id, anomaly_type) in anomalies {
                println!("  - 节点 {:?}: {}", id, anomaly_type);
            }
        } else {
            println!("未检测到异常");
        }
    }

    /// 演示带宽预测功能
    async fn demonstrate_bandwidth_prediction(integration: &AutonomousNetworkIntegration) {
        println!("演示带宽预测功能...");

        // 模拟预测高带宽需求
        integration.manager.make_bandwidth_based_decision(15_000_000.0, 8_000_000.0).await;

        let decisions_count = integration.manager.behaviors.read().await.decisions.len();
        println!("根据带宽预测创建了 {} 个自主决策", decisions_count);

        // 获取推荐
        let recommendations = integration.get_recommendations().await;
        println!("获得 {} 条推荐:", recommendations.len());
        for rec in recommendations {
            println!("  - {}", rec);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_autonomous_integration() {
            demonstrate_enhanced_autonomous_features().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_autonomous_network_manager_creation() {
        let manager = AutonomousNetworkManager::new();

        assert_eq!(manager.confidence_thresholds.critical, 0.9);
        assert_eq!(manager.confidence_thresholds.medium, 0.7);
        assert_eq!(manager.confidence_thresholds.low, 0.5);
        assert_eq!(manager.monitoring_interval_ms, 10000);
    }

    #[tokio::test]
    async fn test_node_score_updates() {
        let manager = AutonomousNetworkManager::new();
        let peer_id = PeerId::random();

        // Update node score
        manager.update_node_score(peer_id, 0.8).await;

        // Verify score was updated
        let node_scores = manager.node_scores.read().await;
        assert_eq!(node_scores.get(&peer_id), Some(&0.8));
    }

    #[tokio::test]
    async fn test_reputation_scoring() {
        let manager = AutonomousNetworkManager::new();
        let peer_id = PeerId::random();

        // Update node score multiple times to build history
        manager.update_node_score(peer_id, 0.7).await;
        manager.update_node_score(peer_id, 0.8).await;
        manager.update_node_score(peer_id, 0.9).await;

        // Calculate reputation based on history
        let reputation = manager.calculate_reputation_score(peer_id).await;

        // Reputation should be a reasonable value between 0 and 1
        assert!(reputation >= 0.0 && reputation <= 1.0);
        println!("Calculated reputation for {:?}: {}", peer_id, reputation);
    }

    #[tokio::test]
    async fn test_bandwidth_based_decision() {
        let manager = AutonomousNetworkManager::new();

        // This should trigger a resource allocation decision
        manager.make_bandwidth_based_decision(15_000_000.0, 5_000_000.0).await;

        // Check that a decision was made
        let behaviors = manager.behaviors.read().await;
        assert!(!behaviors.decisions.is_empty());
    }

    #[tokio::test]
    async fn test_anomaly_detection() {
        let manager = AutonomousNetworkManager::new();
        let peer_id = PeerId::random();

        // Update with consistent scores first
        manager.update_node_score(peer_id, 0.8).await;
        manager.update_node_score(peer_id, 0.85).await;
        manager.update_node_score(peer_id, 0.82).await;

        // Then add an outlier
        manager.update_node_score(peer_id, 0.2).await; // Significant drop

        // Detect anomalies
        let anomalies = manager.detect_node_anomalies().await;

        // Should detect the anomaly
        let anomaly_found = anomalies.iter().any(|(id, _)| id == &peer_id);
        assert!(anomaly_found, "Anomaly should be detected for the peer");
    }
}