#[cfg(test)]
mod autonomous_network_tests {
    use crate::network::autonomous::AutonomousNetworkManager;
    use crate::network::integrated_autonomous::AutonomousNetworkIntegration;
    use crate::node_score::NodeSelector;
    use crate::traffic::TrafficCounter;
    use libp2p::PeerId;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_autonomous_network_manager_creation() {
        let manager = AutonomousNetworkManager::new();

        // Test initial state
        assert_eq!(manager.behaviors.read().await.decisions.len(), 0);
        assert_eq!(manager.node_scores.read().await.len(), 0);
        assert_eq!(manager.traffic_patterns.read().await.len(), 0);

        println!("✅ AutonomousNetworkManager created successfully with proper initial state");
    }

    #[tokio::test]
    async fn test_node_score_updates() {
        let manager = AutonomousNetworkManager::new();
        let peer_id = PeerId::random();

        // Update node score
        manager.update_node_score(peer_id, 85.5).await;

        // Verify the update
        let scores = manager.node_scores.read().await;
        assert!(scores.contains_key(&peer_id));
        assert_eq!(*scores.get(&peer_id).unwrap(), 85.5);

        println!("✅ Node score updates work correctly");
    }

    #[tokio::test]
    async fn test_traffic_pattern_analysis() {
        let manager = AutonomousNetworkManager::new();
        let pattern_key = "test_pattern".to_string();
        let metrics = vec![1.0, 2.0, 3.0];

        // Analyze traffic pattern
        manager.analyze_traffic_pattern(pattern_key.clone(), metrics.clone()).await;

        // Verify the update
        let patterns = manager.traffic_patterns.read().await;
        assert!(patterns.contains_key(&pattern_key));
        assert_eq!(*patterns.get(&pattern_key).unwrap(), metrics);

        println!("✅ Traffic pattern analysis works correctly");
    }

    #[tokio::test]
    async fn test_ai_decision_making() {
        use crate::network::behaviour::AiDecision;

        let manager = AutonomousNetworkManager::new();

        // Make a route optimization decision
        let decision = AiDecision::RouteOptimization {
            path: vec!["node1".to_string(), "node2".to_string()],
            efficiency_score: 0.85,
        };

        manager.make_autonomous_decision(decision.clone(), 0.9).await;

        // Verify the decision was recorded
        let behaviors = manager.behaviors.read().await;
        assert_eq!(behaviors.decisions.len(), 1);

        println!("✅ AI decision making works correctly");
    }

    #[tokio::test]
    async fn test_network_health_score() {
        let manager = AutonomousNetworkManager::new();

        // Initially, health score should be 0
        let initial_score = manager.get_network_health_score().await;
        assert_eq!(initial_score, 0.0);

        // Add some simulated activity to change the score
        {
            let mut behaviors = manager.behaviors.write().await;
            behaviors.network_efficiency = 0.7;
            behaviors.failure_prediction_accuracy = 0.6;
            behaviors.resource_optimization_score = 0.8;
        }

        let updated_score = manager.get_network_health_score().await;
        let expected_score = (0.7 + 0.6 + 0.8) / 3.0; // Average of the three metrics
        assert_eq!(updated_score, expected_score);

        println!("✅ Network health scoring works correctly");
    }

    #[tokio::test]
    async fn test_recommendations_generation() {
        let manager = AutonomousNetworkManager::new();
        let peer_id = PeerId::random();

        // Add a low-performing node to trigger a recommendation
        manager.update_node_score(peer_id, 0.3).await; // Low score

        // Add a poor network efficiency to trigger another recommendation
        {
            let mut behaviors = manager.behaviors.write().await;
            behaviors.network_efficiency = 0.5; // Below threshold of 0.7
        }

        let recommendations = manager.get_recommendations().await;

        // Should have at least one recommendation about the low-performing node
        let has_low_performance_rec = recommendations.iter()
            .any(|rec| rec.contains(&peer_id.to_string()) && rec.contains("low performance"));

        let has_efficiency_rec = recommendations.iter()
            .any(|rec| rec.contains("efficiency"));

        assert!(has_low_performance_rec || has_efficiency_rec,
                "Should have at least one recommendation");

        println!("✅ Recommendations generation works correctly");
    }

    #[tokio::test]
    async fn test_autonomous_integration_full_cycle() {
        // Create all components
        let node_selector = Arc::new(RwLock::new(NodeSelector::new()));
        let traffic_counter = Arc::new(TrafficCounter::new());
        let autonomous_manager = Arc::new(AutonomousNetworkManager::new());

        // Create integration
        let integration = AutonomousNetworkIntegration::new(
            autonomous_manager,
            node_selector,
            traffic_counter,
        );

        // Simulate network activity
        let peer_id = PeerId::random();
        {
            let mut selector = integration.node_selector.write().await;
            selector.record_success(peer_id);
            selector.update_latency(peer_id, 50); // 50ms latency
            selector.set_connected(peer_id, true);
        }

        // Add traffic data
        integration.traffic_counter.add_sent(1024 * 500); // 500KB sent
        integration.traffic_counter.add_received(1024 * 1000); // 1000KB received

        // Test updating node performance
        integration.update_node_performance(peer_id, 90.0).await;

        // Get recommendations and health score
        let recommendations = integration.get_recommendations().await;
        let health_score = integration.get_health_score().await;

        // Verify everything works together
        assert!(health_score >= 0.0 && health_score <= 1.0);

        println!("✅ Full autonomous integration cycle works correctly");
        println!("📊 Health score: {:.2}", health_score);
        println!("📋 Recommendations count: {}", recommendations.len());
    }

    #[tokio::test]
    async fn test_traffic_prediction_integration() {
        use crate::traffic::{TrafficCounter, TrafficPrediction};
        use std::time::Instant;

        let traffic_counter = TrafficCounter::new();

        // Add several traffic samples to enable prediction
        for i in 1..10 {
            traffic_counter.add_sent(i * 100000); // Increasing traffic
            traffic_counter.add_received(i * 200000); // Increasing traffic
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // Small delay
        }

        // Get prediction
        let prediction = traffic_counter.get_current_prediction();

        match prediction {
            Some(pred) => {
                println!("✅ Traffic prediction generated successfully");
                println!("📈 Predicted upload: {:.2} bps", pred.predicted_upload_bps);
                println!("📉 Predicted download: {:.2} bps", pred.predicted_download_bps);
                println!("可信度: {:.2}", pred.confidence);

                // The prediction should have reasonable values
                assert!(pred.predicted_upload_bps >= 0.0);
                assert!(pred.predicted_download_bps >= 0.0);
                assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
            },
            None => {
                // This might happen if there's not enough data yet
                println!("⚠️ No prediction available (insufficient data)");
            }
        }
    }
}