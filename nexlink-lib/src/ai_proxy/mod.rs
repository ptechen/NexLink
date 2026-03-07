pub mod cache;
pub mod coordinator;
pub mod server;

pub use self::cache::*;
pub use self::coordinator::*;
pub use self::server::*;

// Core types and structures
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

/// Represents an AI service endpoint that can handle requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiServiceEndpoint {
    pub id: String,
    pub url: String,
    pub model_types: Vec<String>,  // Supported model types like "gpt-4", "claude", "llama"
    pub max_concurrent_requests: usize,
    pub current_load: usize,
    pub is_healthy: bool,
    pub response_time_avg: f64,  // in milliseconds
    pub capacity_score: f64,     // 0.0 to 1.0, based on availability and performance
}

impl AiServiceEndpoint {
    pub fn new(id: String, url: String, model_types: Vec<String>) -> Self {
        Self {
            id,
            url,
            model_types,
            max_concurrent_requests: 10,
            current_load: 0,
            is_healthy: true,
            response_time_avg: 1000.0,
            capacity_score: 0.8, // Default good score
        }
    }

    /// Calculate the load factor (0.0 to 1.0)
    pub fn load_factor(&self) -> f64 {
        if self.max_concurrent_requests == 0 {
            return 1.0; // Fully loaded if max is 0
        }
        (self.current_load as f64) / (self.max_concurrent_requests as f64)
    }

    /// Update the endpoint's performance metrics
    pub fn update_metrics(&mut self, response_time_ms: f64, _success: bool) {
        // Update average response time with exponential moving average
        self.response_time_avg = 0.7 * self.response_time_avg + 0.3 * response_time_ms;

        // Adjust capacity score based on performance
        self.capacity_score = self.calculate_capacity_score();

        debug!(
            endpoint_id = %self.id,
            response_time = %response_time_ms,
            capacity_score = %self.capacity_score,
            "Updated endpoint metrics"
        );
    }

    fn calculate_capacity_score(&self) -> f64 {
        // Normalize load factor (lower is better)
        let load_score = 1.0 - self.load_factor().min(1.0);

        // Normalize response time (lower is better, assuming 1000ms is baseline poor performance)
        let time_score = (1000.0 / (self.response_time_avg + 1.0)).min(1.0);

        // Weighted combination (may need adjustment based on requirements)
        (0.6 * load_score + 0.4 * time_score).min(1.0)
    }

    /// Increment current load when a request is assigned to this endpoint
    pub fn increment_load(&mut self) {
        self.current_load = std::cmp::min(self.current_load + 1, self.max_concurrent_requests);
    }

    /// Decrement current load when a request is completed
    pub fn decrement_load(&mut self) {
        if self.current_load > 0 {
            self.current_load -= 1;
        }
    }
}

/// Configuration for AI proxy behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProxyConfig {
    pub health_check_interval_secs: u64,
    pub request_timeout_secs: u64,
    pub retry_attempts: usize,
    pub load_balancing_strategy: LoadBalancingStrategy,
}

impl Default for AiProxyConfig {
    fn default() -> Self {
        Self {
            health_check_interval_secs: 30,
            request_timeout_secs: 60,
            retry_attempts: 2,
            load_balancing_strategy: LoadBalancingStrategy::WeightedCapacity,
        }
    }
}

/// Different strategies for load balancing AI requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastLoaded,
    WeightedCapacity,  // Based on capacity score (health, load, response time)
    ModelSpecific,     // Route based on model type
}

/// Main AI Proxy struct that manages routing and load balancing
pub struct AiProxy {
    endpoints: Arc<RwLock<HashMap<String, AiServiceEndpoint>>>,
    config: AiProxyConfig,
    round_robin_index: Arc<RwLock<usize>>,
    pub request_counter: Arc<RwLock<u64>>,
}

impl AiProxy {
    pub fn new(config: AiProxyConfig) -> Self {
        Self {
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            config,
            round_robin_index: Arc::new(RwLock::new(0)),
            request_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Register a new AI service endpoint
    pub async fn register_endpoint(&self, endpoint: AiServiceEndpoint) -> Result<(), String> {
        let mut endpoints = self.endpoints.write().await;
        if endpoints.contains_key(&endpoint.id) {
            return Err(format!("Endpoint with id '{}' already exists", endpoint.id));
        }

        let endpoint_id = endpoint.id.clone();
        endpoints.insert(endpoint_id, endpoint.clone());
        info!("Registered new AI service endpoint: {}", endpoint.id);

        Ok(())
    }

    /// Remove an endpoint
    pub async fn remove_endpoint(&self, endpoint_id: &str) -> Result<AiServiceEndpoint, String> {
        let mut endpoints = self.endpoints.write().await;
        endpoints.remove(endpoint_id)
            .ok_or_else(|| format!("Endpoint with id '{}' not found", endpoint_id))
    }

    /// Get the best endpoint for a specific model type
    pub async fn get_best_endpoint(&self, model_type: Option<&str>) -> Result<String, String> {
        let endpoints = self.endpoints.read().await;
        let available_endpoints: Vec<_> = endpoints
            .values()
            .filter(|ep| ep.is_healthy && (model_type.is_none() ||
                         ep.model_types.iter().any(|mt| mt == model_type.unwrap())))
            .collect();

        if available_endpoints.is_empty() {
            return Err("No healthy endpoints available for requested model type".to_string());
        }

        let best_endpoint = match self.config.load_balancing_strategy {
            LoadBalancingStrategy::RoundRobin => {
                self.select_round_robin(&available_endpoints).await
            },
            LoadBalancingStrategy::LeastLoaded => {
                self.select_least_loaded(&available_endpoints).await
            },
            LoadBalancingStrategy::WeightedCapacity => {
                self.select_weighted_capacity(&available_endpoints).await
            },
            LoadBalancingStrategy::ModelSpecific => {
                // For model-specific routing, we still need to select among available ones
                self.select_weighted_capacity(&available_endpoints).await
            },
        };

        // Increment load for selected endpoint
        {
            let mut endpoints_write = self.endpoints.write().await;
            if let Some(endpoint) = endpoints_write.get_mut(best_endpoint) {
                endpoint.increment_load();
            }
        }

        Ok(best_endpoint.to_string())
    }

    async fn select_round_robin<'a>(&self, endpoints: &[&'a AiServiceEndpoint]) -> &'a str {
        let mut index = self.round_robin_index.write().await;
        let idx = *index % endpoints.len();
        *index += 1;

        endpoints[idx].id.as_str()
    }

    async fn select_least_loaded<'a>(&self, endpoints: &[&'a AiServiceEndpoint]) -> &'a str {
        endpoints
            .iter()
            .min_by(|a, b| a.current_load.cmp(&b.current_load))
            .map(|ep| ep.id.as_str())
            .unwrap_or(endpoints[0].id.as_str())  // fallback
    }

    async fn select_weighted_capacity<'a>(&self, endpoints: &[&'a AiServiceEndpoint]) -> &'a str {
        endpoints
            .iter()
            .max_by(|a, b| a.capacity_score.partial_cmp(&b.capacity_score).unwrap_or(std::cmp::Ordering::Equal))
            .map(|ep| ep.id.as_str())
            .unwrap_or(endpoints[0].id.as_str())  // fallback
    }

    /// Update endpoint metrics after a request completes
    pub async fn update_endpoint_metrics(
        &self,
        endpoint_id: &str,
        response_time_ms: f64,
        success: bool
    ) -> Result<(), String> {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.get_mut(endpoint_id) {
            endpoint.update_metrics(response_time_ms, success);

            // Update load counters
            if !success {
                // Decrement load on failure to allow other requests to use this endpoint
                endpoint.decrement_load();
            }

            Ok(())
        } else {
            Err(format!("Endpoint with id '{}' not found", endpoint_id))
        }
    }

    /// Get statistics about all endpoints
    pub async fn get_statistics(&self) -> HashMap<String, EndpointStats> {
        let endpoints = self.endpoints.read().await;
        let request_count = *self.request_counter.read().await;

        let mut stats = HashMap::new();

        for (id, endpoint) in endpoints.iter() {
            stats.insert(id.clone(), EndpointStats {
                id: id.clone(),
                url: endpoint.url.clone(),
                model_types: endpoint.model_types.clone(),
                current_load: endpoint.current_load,
                max_concurrent_requests: endpoint.max_concurrent_requests,
                load_factor: endpoint.load_factor(),
                response_time_avg: endpoint.response_time_avg,
                capacity_score: endpoint.capacity_score,
                is_healthy: endpoint.is_healthy,
                total_requests_handled: request_count, // This would need to be tracked separately per endpoint
            });
        }

        stats
    }

    /// Increment global request counter
    pub async fn increment_request_counter(&self) {
        let mut counter = self.request_counter.write().await;
        *counter += 1;
    }
}

/// Statistics for an individual endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStats {
    pub id: String,
    pub url: String,
    pub model_types: Vec<String>,
    pub current_load: usize,
    pub max_concurrent_requests: usize,
    pub load_factor: f64,
    pub response_time_avg: f64,
    pub capacity_score: f64,
    pub is_healthy: bool,
    pub total_requests_handled: u64,
}