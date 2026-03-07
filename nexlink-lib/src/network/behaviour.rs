use libp2p::swarm::NetworkBehaviour;
use libp2p::{autonat, identify, ping, relay, rendezvous};
use libp2p_stream as stream;
use serde::{Deserialize, Serialize};

// AI Decision Types for autonomous operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiDecision {
    RouteOptimization { path: Vec<String>, efficiency_score: f64 },
    ResourceAllocation { node_id: String, resources: u64 },
    LoadBalancing { target_nodes: Vec<String> },
    FailurePrediction { failure_probability: f64, affected_nodes: Vec<String> },
    SecurityAlert { threat_level: u8, affected_resources: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousDecision {
    pub decision_type: AiDecision,
    pub confidence: f64,
    pub timestamp: std::time::SystemTime,
    pub recommendation: String,
}

// Autonomous behavior tracker
#[derive(Debug, Clone)]
pub struct AutonomousBehaviors {
    pub decisions: Vec<AutonomousDecision>,
    pub network_efficiency: f64,
    pub failure_prediction_accuracy: f64,
    pub resource_optimization_score: f64,
}

impl Default for AutonomousBehaviors {
    fn default() -> Self {
        Self {
            decisions: Vec::new(),
            network_efficiency: 0.0,
            failure_prediction_accuracy: 0.0,
            resource_optimization_score: 0.0,
        }
    }
}

impl AutonomousBehaviors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn make_decision(&mut self, decision: AiDecision, confidence: f64) {
        let aut_decision = AutonomousDecision {
            decision_type: decision,
            confidence,
            timestamp: std::time::SystemTime::now(),
            recommendation: String::new(),
        };

        self.decisions.push(aut_decision);

        // Limit the number of decisions to prevent memory growth
        if self.decisions.len() > 100 {
            self.decisions.drain(0..50);
        }
    }

    pub fn update_network_efficiency(&mut self, efficiency_change: f64) {
        self.network_efficiency = (self.network_efficiency + efficiency_change).clamp(0.0, 1.0);
    }

    pub fn get_network_health_score(&self) -> f64 {
        // Calculate a composite health score
        (self.network_efficiency + self.failure_prediction_accuracy + self.resource_optimization_score) / 3.0
    }
}

/// Behaviour for client/provider nodes — connects to relay, discovers peers
#[derive(NetworkBehaviour)]
pub struct NexlinkBehaviour {
    pub relay_client: relay::client::Behaviour,
    pub identify: identify::Behaviour,
    pub rendezvous_client: rendezvous::client::Behaviour,
    pub ping: ping::Behaviour,
    pub stream: stream::Behaviour,
    pub autonat: autonat::Behaviour,
}

/// Behaviour for the relay/rendezvous server
#[derive(NetworkBehaviour)]
pub struct RelayBehaviour {
    pub relay: relay::Behaviour,
    pub identify: identify::Behaviour,
    pub rendezvous_server: rendezvous::server::Behaviour,
    pub ping: ping::Behaviour,
    pub autonat: autonat::Behaviour,
}