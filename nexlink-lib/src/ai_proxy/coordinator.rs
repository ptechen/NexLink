//! AI Agent Coordinator
//! Orchestrates the various components of the AI proxy system

use std::sync::Arc;
use tracing::{info, debug, warn, error};

use crate::ai_proxy::{AiProxy, AiProxyConfig, AiServiceEndpoint};
use crate::ai_proxy::cache::{AiResponseCache, CacheConfig};

/// Main coordinator for the AI proxy system
pub struct AiAgentCoordinator {
    pub ai_proxy: Arc<AiProxy>,
    pub response_cache: Arc<AiResponseCache>,
    pub config: AiCoordinatorConfig,
}

/// Configuration for the AI coordinator
#[derive(Debug, Clone)]
pub struct AiCoordinatorConfig {
    pub proxy_config: AiProxyConfig,
    pub cache_config: CacheConfig,
    pub enable_intelligent_routing: bool,
    pub enable_adaptive_scaling: bool,
    pub enable_security_filtering: bool,
}

impl Default for AiCoordinatorConfig {
    fn default() -> Self {
        Self {
            proxy_config: AiProxyConfig::default(),
            cache_config: CacheConfig::default(),
            enable_intelligent_routing: true,
            enable_adaptive_scaling: true,
            enable_security_filtering: true,
        }
    }
}

impl AiAgentCoordinator {
    /// Create a new AI agent coordinator
    pub fn new(config: AiCoordinatorConfig) -> Self {
        let ai_proxy = Arc::new(AiProxy::new(config.proxy_config.clone()));
        let response_cache = Arc::new(AiResponseCache::ai_response_cache());

        Self {
            ai_proxy,
            response_cache,
            config,
        }
    }

    /// Initialize the coordinator with default endpoints
    pub async fn initialize_with_defaults(&self) -> Result<(), String> {
        info!("Initializing AI Agent Coordinator with default configuration");

        // Register default endpoints (these would normally come from configuration)
        let default_endpoints = self.get_default_endpoints();

        for endpoint in default_endpoints {
            if let Err(e) = self.ai_proxy.register_endpoint(endpoint).await {
                warn!("Failed to register default endpoint: {}", e);
            }
        }

        info!("AI Agent Coordinator initialized successfully");
        Ok(())
    }

    /// Get default endpoints (would normally be loaded from config)
    fn get_default_endpoints(&self) -> Vec<AiServiceEndpoint> {
        vec![
            AiServiceEndpoint::new(
                "openai-default".to_string(),
                "https://api.openai.com/v1/chat/completions".to_string(),
                vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()]
            ),
            AiServiceEndpoint::new(
                "anthropic-default".to_string(),
                "https://api.anthropic.com/v1/messages".to_string(),
                vec!["claude-2".to_string(), "claude-3".to_string()]
            ),
        ]
    }

    /// Process an AI request intelligently (with caching and routing)
    pub async fn process_ai_request(
        &self,
        model: &str,
        request_hash: &str,
        process_fn: impl FnOnce() -> Result<serde_json::Value, String>
    ) -> Result<serde_json::Value, String> {
        // Increment global request counter
        self.ai_proxy.increment_request_counter().await;

        // Check cache first
        if self.config.enable_intelligent_routing {
            if let Some(cached_response) = self.response_cache.get_cached_response(model, request_hash).await {
                debug!("Serving cached response for model: {}, hash: {}", model, request_hash);
                return Ok(cached_response);
            }
        }

        // Find the best endpoint for this model
        let endpoint_id = self.ai_proxy.get_best_endpoint(Some(model)).await?;
        debug!("Routing request to endpoint: {}", endpoint_id);

        // Record start time for performance metrics
        let start_time = std::time::Instant::now();

        // Process the request using the provided function
        let result = process_fn();

        let elapsed = start_time.elapsed().as_millis() as f64;

        match result {
            Ok(response) => {
                // Update endpoint metrics
                if let Err(e) = self.ai_proxy.update_endpoint_metrics(&endpoint_id, elapsed, true).await {
                    warn!("Failed to update endpoint metrics: {}", e);
                }

                // Cache the response if successful
                if self.config.enable_intelligent_routing {
                    if let Err(e) = self.response_cache.cache_ai_response(model, request_hash, response.clone(), None).await {
                        warn!("Failed to cache response: {}", e);
                    }
                }

                Ok(response)
            }
            Err(error) => {
                // Update endpoint metrics with failure
                if let Err(e) = self.ai_proxy.update_endpoint_metrics(&endpoint_id, elapsed, false).await {
                    warn!("Failed to update endpoint metrics after error: {}", e);
                }

                error!("Request processing failed: {}", error);
                Err(error)
            }
        }
    }

    /// Get overall system statistics
    pub async fn get_system_stats(&self) -> SystemStats {
        let proxy_stats = self.ai_proxy.get_statistics().await;
        let cache_stats = self.response_cache.stats().await;

        SystemStats {
            timestamp: chrono::Utc::now(),
            proxy_stats,
            cache_stats,
            total_requests: *self.ai_proxy.request_counter.read().await,
        }
    }

    /// Register a new AI service endpoint
    pub async fn register_ai_endpoint(&self, endpoint: AiServiceEndpoint) -> Result<(), String> {
        self.ai_proxy.register_endpoint(endpoint).await
    }

    /// Get health status of the coordinator
    pub async fn health_check(&self) -> HealthStatusDetailed {
        let proxy_stats = self.ai_proxy.get_statistics().await;
        let cache_stats = self.response_cache.stats().await;

        // Determine health based on system metrics
        let mut status = HealthStatus::Healthy;
        if cache_stats.hit_rate < 0.3 {  // Less than 30% cache hit rate
            status = HealthStatus::Degraded;
        }

        let unhealthy_endpoints: Vec<_> = proxy_stats.values()
            .filter(|stats| !stats.is_healthy)
            .map(|stats| stats.id.clone())
            .collect();

        if !unhealthy_endpoints.is_empty() {
            status = HealthStatus::Degraded;
        }

        HealthStatusDetailed {
            status,
            unhealthy_endpoints,
            cache_hit_rate: cache_stats.hit_rate,
            active_endpoints: proxy_stats.len(),
        }
    }
}

/// Overall system statistics
#[derive(Debug, Clone)]
pub struct SystemStats {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub proxy_stats: std::collections::HashMap<String, crate::ai_proxy::EndpointStats>,
    pub cache_stats: crate::ai_proxy::cache::CacheStats,
    pub total_requests: u64,
}

/// Health status of the system
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Detailed health status
#[derive(Debug, Clone)]
pub struct HealthStatusDetailed {
    pub status: HealthStatus,
    pub unhealthy_endpoints: Vec<String>,
    pub cache_hit_rate: f64,
    pub active_endpoints: usize,
}

// Implement basic methods for HealthStatus
impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded)
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let config = AiCoordinatorConfig::default();
        let coordinator = AiAgentCoordinator::new(config);

        assert_eq!(coordinator.response_cache.stats().await.size, 0);
    }

    #[tokio::test]
    async fn test_process_ai_request() {
        let config = AiCoordinatorConfig {
            enable_intelligent_routing: true,
            ..Default::default()
        };
        let coordinator = AiAgentCoordinator::new(config);

        // Add a test endpoint
        let endpoint = AiServiceEndpoint::new(
            "test-endpoint".to_string(),
            "http://test.com".to_string(),
            vec!["gpt-4".to_string()]
        );
        coordinator.register_ai_endpoint(endpoint).await.unwrap();

        // Process a mock request
        let result = coordinator.process_ai_request(
            "gpt-4",
            "test-hash",
            || Ok(serde_json::json!({"response": "test"}))
        ).await;

        assert!(result.is_ok());
        let request_counter = *coordinator.ai_proxy.request_counter.read().await;
        assert_eq!(request_counter, 1);
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = AiCoordinatorConfig::default();
        let coordinator = AiAgentCoordinator::new(config);

        let health = coordinator.health_check().await;
        assert!(health.status.is_healthy());
    }
}