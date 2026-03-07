//! 示例：展示如何在 NexLink 应用中集成自主网络功能
//!
//! 此模块演示了如何将 AutonomousNetworkManager 与现有网络栈集成，
//! 以实现 AI 驱动的自主决策和网络优化。

use crate::network::autonomous::AutonomousNetworkManager;
use crate::network::integrated_autonomous::AutonomousNetworkIntegration;
use crate::node_score::NodeSelector;
use crate::traffic::TrafficCounter;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 启动具有自主功能的完整网络栈
pub async fn start_autonomous_network_stack() -> Result<AutonomousNetworkIntegration, Box<dyn std::error::Error>> {
    // 创建基本组件
    let node_selector = Arc::new(RwLock::new(NodeSelector::new()));
    let traffic_counter = Arc::new(TrafficCounter::new());

    // 创建自主网络管理器
    let autonomous_manager = Arc::new(AutonomousNetworkManager::new());

    // 创建集成层
    let integration = AutonomousNetworkIntegration::new(
        autonomous_manager,
        node_selector,
        traffic_counter,
    );

    // 启动自主监控
    integration.start_monitoring().await;

    println!("✅ 自主网络栈启动成功！");
    println!("📊 网络将自动监控性能并做出 AI 驱动的决策");

    Ok(integration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;

    #[tokio::test]
    async fn test_autonomous_integration() {
        // 启动自主网络栈
        let integration = start_autonomous_network_stack().await.unwrap();

        // 模拟一些网络活动
        {
            let mut selector = integration.node_selector.write().await;
            let peer_id = PeerId::random();
            selector.record_success(peer_id);
            selector.update_latency(peer_id, 100); // 100ms latency
            selector.set_connected(peer_id, true);
        }

        // 添加一些流量数据
        integration.traffic_counter.add_sent(1024 * 100); // 100KB sent
        integration.traffic_counter.add_received(1024 * 200); // 200KB received

        // 更新节点性能
        let peer_id = PeerId::random();
        integration.update_node_performance(peer_id, 85.0).await; // Good performance

        // 获取推荐和健康分数
        let recommendations = integration.get_recommendations().await;
        let health_score = integration.get_health_score().await;

        println!("Recommendations: {:?}", recommendations);
        println!("Health Score: {}", health_score);

        assert!(health_score >= 0.0 && health_score <= 1.0, "Health score should be between 0 and 1");
    }
}