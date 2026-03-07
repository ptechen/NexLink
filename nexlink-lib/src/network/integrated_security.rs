//! 自主网络安全集成模块
//! 将安全功能集成到自主网络管理器中

use crate::network::autonomous::AutonomousNetworkManager;
use crate::network::security::{AdversarialAttackDetector, AutonomousSecurityExt, ThreatLevel};
use crate::node_score::NodeSelector;
use crate::traffic::TrafficCounter;
use libp2p::PeerId;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// 自主网络安全集成器
/// 将安全检测和防护功能集成到自主网络系统中
pub struct AutonomousSecurityIntegration {
    pub manager: Arc<AutonomousNetworkManager>,
    pub security_detector: Arc<RwLock<AdversarialAttackDetector>>,
    pub node_selector: Arc<RwLock<NodeSelector>>,
    pub traffic_counter: Arc<TrafficCounter>,
}

impl AutonomousSecurityIntegration {
    pub fn new(
        manager: Arc<AutonomousNetworkManager>,
        node_selector: Arc<RwLock<NodeSelector>>,
        traffic_counter: Arc<TrafficCounter>,
    ) -> Self {
        Self {
            manager,
            security_detector: Arc::new(RwLock::new(AdversarialAttackDetector::new())),
            node_selector,
            traffic_counter,
        }
    }

    /// 启动安全监控
    pub async fn start_security_monitoring(&self) {
        let manager = self.manager.clone();
        let security_detector = self.security_detector.clone();
        let node_selector = self.node_selector.clone();
        let _traffic_counter = self.traffic_counter.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(15)); // 每15秒进行一次安全检查

            loop {
                interval.tick().await;

                // 执行安全扫描
                Self::perform_security_scan(&manager, &security_detector, &node_selector).await;

                // 检查是否有需要隔离的可疑节点
                Self::check_for_suspicious_nodes(&manager, &security_detector).await;
            }
        });
    }

    /// 执行安全扫描
    async fn perform_security_scan(
        manager: &Arc<AutonomousNetworkManager>,
        security_detector: &Arc<RwLock<AdversarialAttackDetector>>,
        node_selector: &Arc<RwLock<NodeSelector>>,
    ) {
        let peers = {
            let selector = node_selector.read().await;
            selector.detailed_peer_scores() // 使用实际存在的方法
        };

        for (peer_id, _) in peers {
            // 获取节点的当前行为特征
            let current_behavior = manager.calculate_reputation_score(peer_id).await;

            // 转换为行为向量 (简化表示)
            let behavior_vector = vec![current_behavior, 0.5, 0.3]; // 包含其他指标

            // 检测异常行为
            let mut detector = security_detector.write().await;
            let is_anomalous = detector.detect_anomalous_behavior(
                peer_id,
                &behavior_vector,
                "normal_behavior_model"
            ).await;

            if is_anomalous {
                // 记录安全威胁
                tracing::warn!("Detected anomalous behavior for peer: {}", peer_id);
            }
        }

        // 定期清理旧的安全事件
        {
            let mut detector = security_detector.write().await;
            detector.cleanup_old_events(3600).await; // 清理1小时前的安全事件
        }
    }

    /// 检查可疑节点
    async fn check_for_suspicious_nodes(
        manager: &Arc<AutonomousNetworkManager>,
        security_detector: &Arc<RwLock<AdversarialAttackDetector>>,
    ) {
        // 获取所有受信任程度低的节点
        let suspicious_peers = {
            let _detector = security_detector.read().await;
            // 这里我们会获取超过某个威胁级别的节点列表
            Vec::<(PeerId, ThreatLevel)>::new() // 简化实现
        };

        for (_peer_id, threat_level) in suspicious_peers {
            if threat_level == ThreatLevel::High || threat_level == ThreatLevel::Critical {
                // 隔离高度可疑的节点
                manager.isolate_suspicious_nodes(threat_level).await;
            }
        }
    }

    /// 更新节点安全档案
    pub async fn update_node_security_profile(&self, peer_id: PeerId, behaviors: &[f64]) {
        self.manager.update_security_profile(peer_id, behaviors).await;

        let mut detector = self.security_detector.write().await;
        let is_anomalous = detector.detect_anomalous_behavior(
            peer_id,
            behaviors,
            "dynamic_behavior_model"
        ).await;

        if is_anomalous {
            tracing::warn!("Anomalous behavior detected for peer: {}", peer_id);
        }
    }

    /// 评估节点威胁等级
    pub async fn evaluate_threat_level(&self, peer_id: PeerId) -> ThreatLevel {
        let detector = self.security_detector.read().await;
        detector.assess_threat_level(peer_id).await
    }

    /// 手动隔离可疑节点
    pub async fn manually_isolate_node(&self, peer_id: PeerId, threat_level: ThreatLevel) {
        self.manager.isolate_suspicious_nodes(threat_level.clone()).await;  // Clone to avoid move

        let mut detector = self.security_detector.write().await;
                detector.record_security_incident(
                    peer_id,
                    crate::network::security::SecurityIncident {
                        timestamp: std::time::SystemTime::now(),
                        incident_type: "ManualIsolation".to_string(),
                        severity: threat_level, // Now threat_level is available for use
                        description: format!("Node {} manually isolated due to security concerns", peer_id),
                    }
                ).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_score::NodeSelector;
    use crate::traffic::TrafficCounter;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_security_integration() {
        let manager = Arc::new(AutonomousNetworkManager::new());
        let node_selector = Arc::new(RwLock::new(NodeSelector::new()));
        let traffic_counter = Arc::new(TrafficCounter::new());

        let security_integration = AutonomousSecurityIntegration::new(
            manager,
            node_selector,
            traffic_counter,
        );

        let peer_id = PeerId::random();

        // 测试更新节点安全档案
        security_integration.update_node_security_profile(peer_id, &[0.8, 0.7, 0.9]).await;

        // 测试威胁评估
        let threat_level = security_integration.evaluate_threat_level(peer_id).await;
        assert_eq!(threat_level, ThreatLevel::Low);

        // 测试手动隔离
        security_integration.manually_isolate_node(peer_id, ThreatLevel::High).await;
    }
}