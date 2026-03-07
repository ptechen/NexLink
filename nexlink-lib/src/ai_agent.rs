//! Advanced AI Agent Integration for nexlink-node
//! Provides sophisticated AI decision making and autonomous behavior for nodes

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use crate::ai_proxy::{AiAgentCoordinator, AiCoordinatorConfig};
use crate::network::behaviour::{AutonomousBehaviors, AutonomousDecision as OriginalAutonomousDecision};
use crate::node_score::NodeScore;
use libp2p::PeerId;
use std::collections::HashMap;
use std::time::Duration;

/// AI Agent for managing node operations with intelligent decision making
pub struct IntelligentNodeAgent {
    pub coordinator: Arc<AiAgentCoordinator>,
    pub node_preferences: Arc<RwLock<NodePreferences>>,
    pub decision_history: Arc<RwLock<Vec<IntelligentDecision>>>,
    pub autonomous_behaviors: Arc<RwLock<AutonomousBehaviors>>,
}

/// Preferences and settings for the intelligent node
#[derive(Debug, Clone)]
pub struct NodePreferences {
    pub preferred_models: Vec<String>,
    pub resource_limits: ResourceLimits,
    pub trust_threshold: f64,
    pub collaboration_policy: CollaborationPolicy,
}

/// Resource limits for the node
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_concurrent_requests: usize,
    pub max_memory_usage: u64, // in MB
    pub max_bandwidth_percent: u8, // percentage of available bandwidth to use
    pub min_response_quality: f64,
}

/// Policy for node collaboration
#[derive(Debug, Clone)]
pub enum CollaborationPolicy {
    /// Only collaborate with trusted nodes
    TrustedOnly,
    /// Balance between trusted and new nodes
    Balanced,
    /// Opportunistic collaboration with any node
    Opportunistic,
}

/// An intelligent decision made by the AI agent
#[derive(Debug, Clone)]
pub struct IntelligentDecision {
    pub decision_type: IntelligentDecisionType,
    pub confidence: f64,
    pub timestamp: std::time::Instant,
    pub context: DecisionContext,
}

/// Types of intelligent decisions
#[derive(Debug, Clone)]
pub enum IntelligentDecisionType {
    /// Node selection decision
    NodeSelection { selected_node: PeerId, alternatives_evaluated: Vec<PeerId> },
    /// Resource allocation decision
    ResourceAllocation { resource_type: ResourceType, allocated_units: u64 },
    /// Load balancing decision
    LoadBalancing { source_nodes: Vec<PeerId>, destination_nodes: Vec<PeerId> },
    /// Security decision
    Security { action: SecurityAction, affected_nodes: Vec<PeerId> },
    /// Network topology decision
    Topology { action: TopologyAction, impact_assessment: ImpactAssessment },
}

/// Type of resource being allocated
#[derive(Debug, Clone)]
pub enum ResourceType {
    Compute,
    Memory,
    Bandwidth,
    Storage,
}

/// Security action to take
#[derive(Debug, Clone)]
pub enum SecurityAction {
    IsolateNode(PeerId),
    IncreaseMonitoring(PeerId),
    BlockConnection(PeerId),
    RequestAuthentication(PeerId),
}

/// Topology action to take
#[derive(Debug, Clone)]
pub enum TopologyAction {
    ConnectTo(PeerId),
    DisconnectFrom(PeerId),
    AdjustRoute { from: PeerId, to: PeerId },
    FormNewCluster(Vec<PeerId>),
}

/// Impact assessment for decisions
#[derive(Debug, Clone)]
pub struct ImpactAssessment {
    pub performance_impact: f64, // -1.0 to 1.0, negative is bad
    pub security_impact: f64,    // -1.0 to 1.0, negative is bad
    pub resource_impact: f64,    // -1.0 to 1.0, negative is bad
    pub stability_impact: f64,   // -1.0 to 1.0, negative is bad
}

/// Context for making a decision
#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub current_network_state: NetworkState,
    pub available_alternatives: Vec<Alternative>,
    pub constraints: Vec<Constraint>,
    pub objectives: Vec<Objective>,
}

/// Current network state information
#[derive(Debug, Clone)]
pub struct NetworkState {
    pub connected_peers: Vec<PeerId>,
    pub network_latency: HashMap<PeerId, u64>,
    pub peer_capacities: HashMap<PeerId, NodeCapacity>,
    pub current_load: NodeLoad,
    pub security_status: SecurityStatus,
}

/// Capacity information for a node
#[derive(Debug, Clone)]
pub struct NodeCapacity {
    pub compute_units: u64,
    pub memory_available: u64,
    pub bandwidth_available: u64,
    pub reliability_score: f64,
}

/// Current load on a node
#[derive(Debug, Clone)]
pub struct NodeLoad {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub bandwidth_usage: f64,
    pub active_connections: usize,
    pub pending_requests: usize,
}

/// Security status of the network
#[derive(Debug, Clone)]
pub struct SecurityStatus {
    pub threat_level: u8, // 0-100 scale
    pub trust_scores: HashMap<PeerId, f64>,
    pub security_events: Vec<SecurityEvent>,
}

/// Security event that occurred
#[derive(Debug, Clone)]
pub struct SecurityEvent {
    pub event_type: SecurityEventType,
    pub severity: u8,
    pub timestamp: std::time::Instant,
    pub details: String,
}

/// Type of security event
#[derive(Debug, Clone)]
pub enum SecurityEventType {
    UnauthorizedAccessAttempt,
    DataIntegrityViolation,
    ResourceExhaustion,
    ConnectionSpam,
    IdentityVerificationFailure,
}

/// Alternative option for decision making
#[derive(Debug, Clone)]
pub struct Alternative {
    pub id: String,
    pub score: f64,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub estimated_outcome: OutcomeEstimate,
}

/// Estimated outcome of choosing an alternative
#[derive(Debug, Clone)]
pub struct OutcomeEstimate {
    pub success_probability: f64,
    pub expected_benefit: f64,
    pub risk_level: f64, // 0.0 to 1.0
    pub resource_requirements: HashMap<ResourceType, u64>,
}

/// Constraint that affects decision making
#[derive(Debug, Clone)]
pub enum Constraint {
    ResourceLimit { resource_type: ResourceType, max_amount: u64 },
    TimeLimit { max_duration: Duration },
    QualityThreshold { min_score: f64 },
    SecurityRequirement { min_trust_score: f64 },
}

/// Objective for the decision
#[derive(Debug, Clone)]
pub enum Objective {
    MaximizePerformance,
    MinimizeResourceUsage,
    MaximizeSecurity,
    MaximizeNetworkStability,
    OptimizeCostEfficiency,
}

impl IntelligentNodeAgent {
    /// Create a new intelligent node agent
    pub fn new() -> Self {
        let coordinator_config = AiCoordinatorConfig::default();
        let coordinator = Arc::new(AiAgentCoordinator::new(coordinator_config));

        let autonomous_behaviors = Arc::new(RwLock::new(AutonomousBehaviors::new()));

        let preferences = Arc::new(RwLock::new(NodePreferences {
            preferred_models: vec!["gpt-4".to_string(), "claude-3".to_string()],
            resource_limits: ResourceLimits {
                max_concurrent_requests: 50,
                max_memory_usage: 1024, // 1GB
                max_bandwidth_percent: 80,
                min_response_quality: 0.7,
            },
            trust_threshold: 0.6,
            collaboration_policy: CollaborationPolicy::Balanced,
        }));

        Self {
            coordinator,
            node_preferences: preferences,
            decision_history: Arc::new(RwLock::new(Vec::new())),
            autonomous_behaviors,
        }
    }

    /// Process an AI request intelligently with enhanced decision making
    pub async fn process_intelligent_request(
        &self,
        model: &str,
        request_hash: &str,
        process_fn: impl FnOnce() -> Result<serde_json::Value, String>,
    ) -> Result<serde_json::Value, String> {
        info!("Processing intelligent request for model: {}", model);

        // Make a preliminary decision about which node to use
        let network_state = self.get_current_network_state().await;
        let decision = self.make_node_selection_decision(&network_state, model).await;

        if let Some(decision) = decision {
            self.record_decision(decision).await;
        }

        // Process through the coordinator as usual
        self.coordinator
            .process_ai_request(model, request_hash, process_fn)
            .await
    }

    /// Get the current network state
    async fn get_current_network_state(&self) -> NetworkState {
        // This would integrate with the actual network state
        // For now, returning a default state
        NetworkState {
            connected_peers: vec![],
            network_latency: HashMap::new(),
            peer_capacities: HashMap::new(),
            current_load: NodeLoad {
                cpu_usage: 0.0,
                memory_usage: 0.0,
                bandwidth_usage: 0.0,
                active_connections: 0,
                pending_requests: 0,
            },
            security_status: SecurityStatus {
                threat_level: 0,
                trust_scores: HashMap::new(),
                security_events: vec![],
            },
        }
    }

    /// Make a node selection decision
    async fn make_node_selection_decision(
        &self,
        network_state: &NetworkState,
        _model: &str,
    ) -> Option<IntelligentDecision> {
        // In a real implementation, this would analyze the network state
        // and decide which node is best for the given model

        if network_state.connected_peers.is_empty() {
            return None;
        }

        // Placeholder logic - in reality this would be much more sophisticated
        let selected_node = network_state.connected_peers.first().copied()?;

        let decision_context = DecisionContext {
            current_network_state: network_state.clone(),
            available_alternatives: vec![],
            constraints: vec![],
            objectives: vec![Objective::MaximizePerformance],
        };

        Some(IntelligentDecision {
            decision_type: IntelligentDecisionType::NodeSelection {
                selected_node,
                alternatives_evaluated: network_state.connected_peers.clone(),
            },
            confidence: 0.8,
            timestamp: std::time::Instant::now(),
            context: decision_context,
        })
    }

    /// Record a decision in the history
    async fn record_decision(&self, decision: IntelligentDecision) {
        let mut history = self.decision_history.write().await;
        history.push(decision);

        // Limit history size to prevent unbounded growth
        if history.len() > 1000 {
            history.drain(0..500);
        }
    }

    /// Integrate with autonomous network behaviors
    pub async fn synchronize_with_autonomous_network(&self, _peer_id: PeerId, node_score: NodeScore) {
        // In real implementation, update autonomous behaviors based on node performance
        // For now we'll just update the network efficiency
        let mut behaviors = self.autonomous_behaviors.write().await;
        behaviors.update_network_efficiency(node_score.success_rate());
    }

    /// Handle a decision from the autonomous controller
    #[allow(dead_code)] // Suppress unused method warning
    async fn handle_autonomous_decision(&self, decision: OriginalAutonomousDecision) {
        // Convert to intelligent decision and record it
        let intelligent_decision = IntelligentDecision {
            decision_type: self.convert_autonomous_to_intelligent_decision(&decision),
            confidence: decision.confidence,
            timestamp: std::time::Instant::now(),
            context: DecisionContext {
                current_network_state: self.get_current_network_state().await,
                available_alternatives: vec![],
                constraints: vec![],
                objectives: vec![Objective::MaximizeNetworkStability],
            },
        };

        self.record_decision(intelligent_decision).await;
    }

    /// Convert autonomous decision type to intelligent decision type
    #[allow(dead_code)] // Suppress unused method warning
    fn convert_autonomous_to_intelligent_decision(&self, decision: &OriginalAutonomousDecision) -> IntelligentDecisionType {
        use crate::network::behaviour::AiDecision;
        match &decision.decision_type {
            AiDecision::RouteOptimization { path: _, efficiency_score: _ } => {
                IntelligentDecisionType::Topology {
                    action: TopologyAction::FormNewCluster(vec![]), // Placeholder
                    impact_assessment: ImpactAssessment {
                        performance_impact: decision.confidence,
                        security_impact: 0.5,
                        resource_impact: 0.2,
                        stability_impact: 0.8,
                    },
                }
            },
            AiDecision::ResourceAllocation { node_id: _, resources } => {
                IntelligentDecisionType::ResourceAllocation {
                    resource_type: ResourceType::Compute,
                    allocated_units: *resources,
                }
            },
            AiDecision::LoadBalancing { target_nodes } => {
                IntelligentDecisionType::LoadBalancing {
                    source_nodes: vec![], // Would be determined in actual implementation
                    destination_nodes: target_nodes.iter().map(|_s| PeerId::random()).collect(), // Convert string to PeerId
                }
            },
            AiDecision::FailurePrediction { failure_probability: _, affected_nodes } => {
                IntelligentDecisionType::Security {
                    action: SecurityAction::IncreaseMonitoring(PeerId::random()), // Placeholder
                    affected_nodes: affected_nodes.iter().map(|_s| PeerId::random()).collect(), // Convert string to PeerId
                }
            },
            AiDecision::SecurityAlert { threat_level: _, affected_resources: _ } => {
                IntelligentDecisionType::Security {
                    action: SecurityAction::IsolateNode(PeerId::random()), // Placeholder
                    affected_nodes: vec![], // Would come from context
                }
            },
        }
    }

    /// Update node preferences
    pub async fn update_preferences(&self, new_preferences: NodePreferences) {
        let mut prefs = self.node_preferences.write().await;
        *prefs = new_preferences;
    }

    /// Get current node preferences
    pub async fn get_preferences(&self) -> NodePreferences {
        self.node_preferences.read().await.clone()
    }

    /// Get recent decisions
    pub async fn get_recent_decisions(&self, count: usize) -> Vec<IntelligentDecision> {
        let history = self.decision_history.read().await;
        history.iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    /// Get the overall intelligence score of the node
    pub async fn get_intelligence_score(&self) -> f64 {
        // Calculate based on various factors:
        // - Number of smart decisions made
        // - Effectiveness of those decisions
        // - Adaptability to changing conditions

        let decision_count = self.decision_history.read().await.len();

        // Placeholder calculation - in reality this would be more sophisticated
        let base_score = (decision_count.min(100) as f64) / 100.0;

        // Adjust based on autonomous behaviors network health assessment
        let network_health = {
            let behaviors = self.autonomous_behaviors.read().await;
            behaviors.get_network_health_score()
        };

        // Combine scores (this is a simplified approach)
        (base_score + network_health) / 2.0
    }
}

impl Default for IntelligentNodeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_intelligent_node_agent_creation() {
        let agent = IntelligentNodeAgent::new();

        assert_eq!(agent.decision_history.read().await.len(), 0);
        assert!(agent.get_intelligence_score().await >= 0.0);
    }

    #[tokio::test]
    async fn test_process_intelligent_request() {
        let agent = IntelligentNodeAgent::new();

        let result = agent.process_intelligent_request(
            "gpt-4",
            "test-hash",
            || Ok(serde_json::json!({"response": "test"})),
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_preferences() {
        let agent = IntelligentNodeAgent::new();

        let new_prefs = NodePreferences {
            preferred_models: vec!["custom-model".to_string()],
            resource_limits: ResourceLimits {
                max_concurrent_requests: 100,
                max_memory_usage: 2048,
                max_bandwidth_percent: 90,
                min_response_quality: 0.8,
            },
            trust_threshold: 0.7,
            collaboration_policy: CollaborationPolicy::TrustedOnly,
        };

        agent.update_preferences(new_prefs.clone()).await;
        let retrieved_prefs = agent.get_preferences().await;

        assert_eq!(retrieved_prefs.preferred_models, vec!["custom-model"]);
        assert_eq!(retrieved_prefs.resource_limits.max_concurrent_requests, 100);
    }

    #[tokio::test]
    async fn test_get_recent_decisions() {
        let agent = IntelligentNodeAgent::new();

        // Manually add a decision for testing
        let decision = IntelligentDecision {
            decision_type: IntelligentDecisionType::ResourceAllocation {
                resource_type: ResourceType::Compute,
                allocated_units: 100,
            },
            confidence: 0.9,
            timestamp: std::time::Instant::now(),
            context: DecisionContext {
                current_network_state: NetworkState {
                    connected_peers: vec![],
                    network_latency: HashMap::new(),
                    peer_capacities: HashMap::new(),
                    current_load: NodeLoad {
                        cpu_usage: 0.0,
                        memory_usage: 0.0,
                        bandwidth_usage: 0.0,
                        active_connections: 0,
                        pending_requests: 0,
                    },
                    security_status: SecurityStatus {
                        threat_level: 0,
                        trust_scores: HashMap::new(),
                        security_events: vec![],
                    },
                },
                available_alternatives: vec![],
                constraints: vec![],
                objectives: vec![Objective::MaximizePerformance],
            },
        };

        agent.record_decision(decision).await;

        let recent_decisions = agent.get_recent_decisions(5).await;
        assert_eq!(recent_decisions.len(), 1);
        assert_eq!(recent_decisions[0].confidence, 0.9);
    }
}