//! HTTP server for the AI proxy
//! Provides an HTTP interface for routing AI requests to various backend services

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio;
use tracing::{info, debug, error, warn};

use crate::ai_proxy::{AiAgentCoordinator, AiCoordinatorConfig};

/// Request to process an AI query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
}

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Response from an AI service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

/// Choice in AI response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

/// Usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// State held by the server
pub struct AppState {
    pub ai_coordinator: Arc<AiAgentCoordinator>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Statistics response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub uptime: String,
    pub total_requests: u64,
    pub cache_hit_rate: f64,
    pub active_endpoints: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Start the AI proxy HTTP server
pub async fn start_server(port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting AI Proxy server on port {}", port);

    // Create AI coordinator
    let config = AiCoordinatorConfig::default();
    let ai_coordinator = Arc::new(AiAgentCoordinator::new(config));

    // Initialize with default endpoints
    if let Err(e) = ai_coordinator.initialize_with_defaults().await {
        warn!("Failed to initialize with default endpoints: {}", e);
    }

    // Create application state
    let app_state = Arc::new(AppState {
        ai_coordinator,
    });

    // Build our application with some routes
    let app = Router::new()
        .route("/", post(handle_ai_request))
        .route("/v1/chat/completions", post(handle_ai_request))
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/endpoints", get(list_endpoints).post(register_endpoint))
        .with_state(app_state);

    // Run the server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    info!("AI Proxy server listening on port {}", port);

    axum::serve(listener, app)
        .await
        .unwrap();

    Ok(())
}

/// Handler for AI requests
async fn handle_ai_request(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AiRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Received AI request for model: {}", request.model);

    // Generate a hash of the request for caching purposes
    let request_content = format!("{:?}", request.messages);  // Simplified hashing
    let request_hash = format!("{:x}", md5::compute(&request_content));

    // Process the request through the coordinator
    let response_result = state
        .ai_coordinator
        .process_ai_request(
            &request.model,
            &request_hash,
            || {
                // Simulate a call to the backend service
                // In a real implementation, this would call the actual AI service
                simulate_backend_call(&request)
            }
        )
        .await;

    match response_result {
        Ok(response) => {
            // Convert to proper response format
            Ok((StatusCode::OK, Json(response)))
        }
        Err(e) => {
            error!("Error processing AI request: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Simulates a call to a backend AI service
fn simulate_backend_call(request: &AiRequest) -> Result<Value, String> {
    // In a real implementation, this would call the actual AI service
    // For simulation purposes, we'll return a mock response

    let response = AiResponse {
        id: format!("chatcmpl-{}", rand::random::<u64>()),
        model: request.model.clone(),
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: "assistant".to_string(),
                content: "This is a simulated response from the AI model.".to_string(),
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: request.messages.iter().map(|m| m.content.len() as u32).sum::<u32>(),
            completion_tokens: 10,
            total_tokens: (10 + request.messages.iter().map(|m| m.content.len() as u32).sum::<u32>()),
        },
    };

    Ok(serde_json::to_value(response).map_err(|e| e.to_string())?)
}

/// Health check endpoint
async fn health_check(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let response = HealthCheckResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now(),
    };

    Json(response)
}

/// Get statistics endpoint
async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let system_stats = state.ai_coordinator.get_system_stats().await;

    let response = StatsResponse {
        uptime: "placeholder".to_string(), // Would calculate from server start time
        total_requests: system_stats.total_requests,
        cache_hit_rate: system_stats.cache_stats.hit_rate,
        active_endpoints: system_stats.proxy_stats.len(),
        timestamp: chrono::Utc::now(),
    };

    Json(response)
}

/// List registered endpoints
async fn list_endpoints(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let proxy_stats = state.ai_coordinator.ai_proxy.get_statistics().await;
    Json(proxy_stats)
}

/// Register a new endpoint
async fn register_endpoint(
    State(state): State<Arc<AppState>>,
    Json(endpoint_data): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Parse endpoint data
    let id = endpoint_data["id"].as_str().unwrap_or_default().to_string();
    let url = endpoint_data["url"].as_str().unwrap_or_default().to_string();
    let model_types: Vec<String> = endpoint_data["model_types"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    if id.is_empty() || url.is_empty() || model_types.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing required fields".to_string());
    }

    let endpoint = crate::ai_proxy::AiServiceEndpoint::new(id, url, model_types);

    match state.ai_coordinator.register_ai_endpoint(endpoint).await {
        Ok(()) => (StatusCode::OK, "Endpoint registered successfully".to_string()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tokio;
    use tower::ServiceExt; // for `app.oneshot()`

    #[tokio::test]
    async fn test_health_check() {
        let config = AiCoordinatorConfig::default();
        let ai_coordinator = Arc::new(AiAgentCoordinator::new(config));
        let app_state = Arc::new(AppState { ai_coordinator });
        let app = Router::new()
            .route("/health", get(health_check))
            .with_state(app_state);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_ai_request_route() {
        let config = AiCoordinatorConfig::default();
        let ai_coordinator = Arc::new(AiAgentCoordinator::new(config));
        let app_state = Arc::new(AppState { ai_coordinator });
        let app = Router::new()
            .route("/", post(handle_ai_request))
            .with_state(app_state);

        let request_json = json!({
            "model": "gpt-4",
            "messages": [
                {
                    "role": "user",
                    "content": "Hello!"
                }
            ]
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header("content-type", "application/json")
                    .body(Body::from(request_json.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}