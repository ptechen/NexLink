use anyhow::{Context, Result};
use libp2p::futures::AsyncWriteExt as FuturesAsyncWriteExt;
use libp2p::PeerId;
use libp2p_stream as stream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use crate::traffic::TrafficCounter;
use crate::node_score::NodeSelector;

/// Max size for HTTP request line (e.g. "CONNECT host:port HTTP/1.1\r\n") — 8 KB
const MAX_REQUEST_LINE_LEN: usize = 8192;
/// Max size for HTTP headers block — 64 KB
const MAX_HEADERS_LEN: usize = 65536;
/// Timeout for reading the full HTTP request line + headers — 30 seconds
const HTTP_PARSE_TIMEOUT: Duration = Duration::from_secs(30);

/// Represents detected content type for intelligent routing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentType {
    Text(String),           // MIME type for text content
    Binary(String),         // MIME type for binary content
    Streaming(String),      // MIME type for streaming content
    Unknown,
}

/// Represents content analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAnalysis {
    pub content_type: ContentType,
    pub estimated_size: Option<u64>,
    pub priority: u8,        // Priority level 0-10 (higher = more urgent)
    pub sensitive: bool,     // Whether content is privacy-sensitive
    pub ai_model_request: bool, // Whether this is an AI model request
}

/// AI-powered content analyzer
#[derive(Debug, Clone)]
pub struct ContentAnalyzer {
    // Patterns to detect AI-related content
    ai_patterns: Vec<String>,
    // Priorities for different content types
    content_priorities: HashMap<String, u8>,
}

impl ContentAnalyzer {
    pub fn new() -> Self {
        let mut content_priorities = HashMap::new();
        content_priorities.insert("application/json".to_string(), 8);
        content_priorities.insert("text/event-stream".to_string(), 9); // High priority for streaming
        content_priorities.insert("application/octet-stream".to_string(), 5);

        let ai_patterns = vec![
            "openai".to_string(),
            "anthropic".to_string(),
            "ai".to_string(),
            "model".to_string(),
            "embedding".to_string(),
            "completion".to_string(),
            "gpt".to_string(),
        ];

        Self {
            ai_patterns,
            content_priorities,
        }
    }

    /// Analyze content to determine its type and properties
    pub fn analyze_content(&self, data: &[u8], target_url: &str) -> ContentAnalysis {
        // Basic content type detection
        let content_type = self.detect_content_type(data);

        // Estimate content size if not known
        let estimated_size = Some(data.len() as u64);

        // Check for AI-related content
        let ai_model_request = self.ai_patterns.iter().any(|pattern|
            target_url.to_lowercase().contains(pattern) ||
            std::str::from_utf8(data).unwrap_or("").to_lowercase().contains(pattern)
        );

        // Determine priority based on content type
        let priority = match content_type {
            ContentType::Streaming(_) => 9,
            ContentType::Text(ref mime) => {
                self.content_priorities.get(mime).copied().unwrap_or(5)
            },
            ContentType::Binary(ref mime) => {
                self.content_priorities.get(mime).copied().unwrap_or(4)
            },
            ContentType::Unknown => if ai_model_request { 8 } else { 3 },
        };

        // Basic sensitivity detection (for demonstration)
        let sensitive = target_url.contains("api")
            && (target_url.contains("auth") || target_url.contains("token"));

        ContentAnalysis {
            content_type,
            estimated_size,
            priority,
            sensitive,
            ai_model_request,
        }
    }

    fn detect_content_type(&self, data: &[u8]) -> ContentType {
        // Try to parse as UTF-8 text
        if let Ok(text) = std::str::from_utf8(data) {
            if text.starts_with('{') || text.starts_with('[') {
                // Likely JSON
                return ContentType::Text("application/json".to_string());
            } else if text.contains("<html") || text.contains("<!DOCTYPE") {
                // HTML
                return ContentType::Text("text/html".to_string());
            } else if text.starts_with("HTTP/") {
                // HTTP response
                return ContentType::Text("application/http".to_string());
            } else if text.contains("event: ") {
                // Server-sent events
                return ContentType::Streaming("text/event-stream".to_string());
            } else {
                // Generic text
                return ContentType::Text("text/plain".to_string());
            }
        }

        // Binary detection heuristics
        if data.len() >= 8 {
            // Check for common file signatures
            if data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
                return ContentType::Binary("image/png".to_string());
            } else if data[0..3] == [0xFF, 0xD8, 0xFF] {
                return ContentType::Binary("image/jpeg".to_string());
            } else if data[0..4] == [0x47, 0x49, 0x46, 0x38] {
                return ContentType::Binary("image/gif".to_string());
            }
        }

        ContentType::Unknown
    }
}

/// Context-aware proxy settings
#[derive(Debug, Clone)]
pub struct CognitiveProxyConfig {
    pub max_connections: usize,
    pub buffer_size: usize,
    pub content_analysis_enabled: bool,
    pub ai_priority_boost: bool,
    pub privacy_mode: bool,
}

impl Default for CognitiveProxyConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            buffer_size: 8192,
            content_analysis_enabled: true,
            ai_priority_boost: true,
            privacy_mode: false,
        }
    }
}

/// Cognitive proxy with content understanding and context-aware processing
pub struct CognitiveProxy {
    #[allow(dead_code)]
    config: CognitiveProxyConfig,
    #[allow(dead_code)]
    content_analyzer: ContentAnalyzer,
    #[allow(dead_code)]
    node_selector: Arc<RwLock<NodeSelector>>,
    #[allow(dead_code)]
    active_streams: Arc<RwLock<HashMap<String, ContentAnalysis>>>,
}

use super::PROXY_PROTOCOL;

/// Read an HTTP request line from a socket that already had its first byte consumed.
/// Enforces a max size limit. Returns the full request line including trailing \r\n.
async fn read_http_request_line(
    socket: &mut tokio::net::TcpStream,
    first_byte: u8,
) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(256);
    buf.push(first_byte);

    let mut prev_cr = false;
    let mut byte = [0u8; 1];

    loop {
        socket.read_exact(&mut byte).await?;
        buf.push(byte[0]);

        if prev_cr && byte[0] == b'\n' {
            break;
        }
        prev_cr = byte[0] == b'\r';

        if buf.len() >= MAX_REQUEST_LINE_LEN {
            anyhow::bail!("HTTP request line exceeds {} byte limit", MAX_REQUEST_LINE_LEN);
        }
    }
    Ok(buf)
}

/// Read HTTP headers until \r\n\r\n, enforcing a max size limit.
async fn read_http_headers(socket: &mut tokio::net::TcpStream) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];

    loop {
        socket.read_exact(&mut byte).await?;
        buf.push(byte[0]);

        if buf.len() >= 4 {
            let len = buf.len();
            if buf[len - 4..] == [b'\r', b'\n', b'\r', b'\n'] {
                break;
            }
        }

        if buf.len() >= MAX_HEADERS_LEN {
            anyhow::bail!("HTTP headers exceed {} byte limit", MAX_HEADERS_LEN);
        }
    }
    Ok(buf)
}

/// Parse an HTTP CONNECT request (request line + headers) with timeout and size limits.
/// Returns the target host:port string.
async fn parse_http_connect(
    socket: &mut tokio::net::TcpStream,
    first_byte: u8,
) -> Result<String> {
    let result = tokio::time::timeout(HTTP_PARSE_TIMEOUT, async {
        let request_line = read_http_request_line(socket, first_byte).await?;

        let request_line_str = String::from_utf8_lossy(&request_line);
        let parts: Vec<&str> = request_line_str
            .trim_end_matches("\r\n")
            .trim()
            .split_whitespace()
            .collect();

        if parts.len() < 3 || parts[0] != "CONNECT" {
            socket
                .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")
                .await?;
            anyhow::bail!("Invalid request: {}", request_line_str);
        }

        let target = parts[1].to_string();

        // Read and discard headers
        let _headers = read_http_headers(socket).await?;

        Ok(target)
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => {
            anyhow::bail!("HTTP request parsing timed out after {:?}", HTTP_PARSE_TIMEOUT);
        }
    }
}

/// Starts a unified proxy server that handles both SOCKS5 and HTTP CONNECT protocols on a single port.
/// Automatically detects the protocol based on the initial bytes of the connection.
pub async fn start_unified_proxy(
    port: u16,
    provider_peer: PeerId,
    control: stream::Control,
    traffic: TrafficCounter,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    info!(port, "Unified proxy listening on single port");

    loop {
        let (socket, addr) = listener.accept().await?;
        let mut ctl = control.clone();
        let peer = provider_peer;
        let counter = traffic.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_unified_connection(socket, peer, &mut ctl, &counter).await {
                warn!(%addr, "Unified proxy error: {e:#}");
            }
        });
    }
}

/// Handles a single incoming connection, detecting whether it's SOCKS5 or HTTP CONNECT
/// and routing to the appropriate handler.
async fn handle_unified_connection(
    mut socket: tokio::net::TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
) -> Result<()> {
    // Read the first byte to determine protocol type
    // read_exact returns Err(UnexpectedEof) if the connection is empty
    let mut first_byte_buffer = [0u8; 1];
    socket.read_exact(&mut first_byte_buffer).await?;

    let first_byte = first_byte_buffer[0];

    // Determine protocol based on first byte
    // SOCKS5: First byte is 0x05 (version indicator)
    // HTTP CONNECT: First byte is 'C' (67) for 'CONNECT', or other letters for 'GET', 'POST', etc.
    if first_byte == 0x05 {
        // This is a SOCKS5 connection
        handle_socks5_from_start(socket, provider_peer, control, traffic).await
    } else {
        // This is an HTTP connection - we need to reconstruct the request
        handle_http_connect_from_start(socket, first_byte, provider_peer, control, traffic).await
    }
}


/// Handle SOCKS5 from the beginning of the connection, having already read the first byte
async fn handle_socks5_from_start(
    mut socket: tokio::net::TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
) -> Result<()> {
    // We already have the first byte as 0x05, now read the rest of the greeting
    let mut nmethods_byte = [0u8; 1];
    socket.read_exact(&mut nmethods_byte).await?;
    let nmethods = nmethods_byte[0] as usize;
    let mut methods = vec![0u8; nmethods];
    socket.read_exact(&mut methods).await?;

    // Reply: no auth required (method 0x00)
    socket.write_all(&[0x05, 0x00]).await?;

    // Read connect request: [ver, cmd, rsv, atyp, addr..., port]
    let mut req_header = [0u8; 4];
    socket.read_exact(&mut req_header).await?;
    if req_header[0] != 0x05 || req_header[1] != 0x01 {
        // Only support CONNECT (0x01)
        socket
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;
        anyhow::bail!("Unsupported SOCKS5 command: {}", req_header[1]);
    }

    let target = match req_header[3] {
        0x01 => {
            // IPv4
            let mut addr = [0u8; 4];
            socket.read_exact(&mut addr).await?;
            let mut port_bytes = [0u8; 2];
            socket.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            format!("{}.{}.{}.{}:{}", addr[0], addr[1], addr[2], addr[3], port)
        }
        0x03 => {
            // Domain name
            let mut len = [0u8; 1];
            socket.read_exact(&mut len).await?;
            let mut domain = vec![0u8; len[0] as usize];
            socket.read_exact(&mut domain).await?;
            let mut port_bytes = [0u8; 2];
            socket.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            format!("{}:{}", String::from_utf8_lossy(&domain), port)
        }
        0x04 => {
            // IPv6
            let mut addr = [0u8; 16];
            socket.read_exact(&mut addr).await?;
            let mut port_bytes = [0u8; 2];
            socket.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            let ipv6 = std::net::Ipv6Addr::from(addr);
            format!("[{ipv6}]:{port}")
        }
        _ => anyhow::bail!("Unknown SOCKS5 address type: {}", req_header[3]),
    };

    info!(%target, "SOCKS5 CONNECT");

    // Open P2P stream to provider node
    let mut p2p_stream = control
        .open_stream(provider_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    // Send target as first line of P2P protocol
    p2p_stream
        .write_all(format!("{target}\n").as_bytes())
        .await?;
    p2p_stream.flush().await?;

    // Reply success to SOCKS5 client
    //    [ver, rep(success), rsv, atyp(ipv4), bind_addr(0.0.0.0), bind_port(0)]
    socket
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    // Bidirectional relay with proper half-close to avoid dropping in-flight data
    traffic.inc_connections();
    let mut p2p_compat = p2p_stream.compat();
    crate::traffic::relay_bidirectional(
        &mut socket,
        &mut p2p_compat,
        &traffic.bytes_sent,
        &traffic.bytes_received,
    )
    .await;
    traffic.dec_connections();

    Ok(())
}

/// Handle HTTP CONNECT from the beginning of the connection, with the first byte already read
async fn handle_http_connect_from_start(
    mut socket: tokio::net::TcpStream,
    first_byte: u8,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
) -> Result<()> {
    // Parse HTTP CONNECT with timeout and size limits (Slowloris / OOM protection)
    let target = parse_http_connect(&mut socket, first_byte).await?;

    info!(%target, "HTTP CONNECT");

    // Open P2P stream to provider node
    let mut p2p_stream = control
        .open_stream(provider_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    // Send target as first line of P2P protocol
    p2p_stream
        .write_all(format!("{target}\n").as_bytes())
        .await?;
    p2p_stream.flush().await?;

    // Reply 200 Connection Established to client
    socket
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    // Bidirectional relay with proper half-close to avoid dropping in-flight data
    traffic.inc_connections();
    let mut p2p_compat = p2p_stream.compat();
    crate::traffic::relay_bidirectional(
        &mut socket,
        &mut p2p_compat,
        &traffic.bytes_sent,
        &traffic.bytes_received,
    )
    .await;
    traffic.dec_connections();

    Ok(())
}

/// Starts a cognitive proxy server that handles both SOCKS5 and HTTP CONNECT protocols with AI-powered routing
pub async fn start_cognitive_proxy(
    port: u16,
    node_selector: Arc<RwLock<NodeSelector>>,
    traffic: TrafficCounter,
    config: CognitiveProxyConfig,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    let content_analyzer = ContentAnalyzer::new();

    info!(port, "Cognitive proxy listening on single port with AI capabilities");

    loop {
        let (socket, addr) = listener.accept().await?;
        let node_selector_clone = node_selector.clone();
        let counter = traffic.clone();
        let config_clone = config.clone();
        let content_analyzer_clone = content_analyzer.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_cognitive_connection(
                socket,
                node_selector_clone,
                &counter,
                &config_clone,
                content_analyzer_clone,
            ).await {
                warn!(%addr, "Cognitive proxy error: {e:#}");
            }
        });
    }
}

/// Handles a single incoming connection with cognitive capabilities
async fn handle_cognitive_connection(
    mut socket: tokio::net::TcpStream,
    node_selector: Arc<RwLock<NodeSelector>>,
    traffic: &TrafficCounter,
    config: &CognitiveProxyConfig,
    content_analyzer: ContentAnalyzer,
) -> Result<()> {
    // Read the first byte to determine protocol type
    // read_exact returns Err(UnexpectedEof) if the connection is empty
    let mut first_byte_buffer = [0u8; 1];
    socket.read_exact(&mut first_byte_buffer).await?;

    let first_byte = first_byte_buffer[0];

    // Determine protocol based on first byte
    if first_byte == 0x05 {
        // This is a SOCKS5 connection
        handle_cognitive_socks5_from_start(
            socket,
            node_selector,
            traffic,
            config,
            content_analyzer,
        ).await
    } else {
        // This is an HTTP connection
        handle_cognitive_http_connect_from_start(
            socket,
            first_byte,
            node_selector,
            traffic,
            config,
            content_analyzer,
        ).await
    }
}

/// Handle SOCKS5 with cognitive capabilities
async fn handle_cognitive_socks5_from_start(
    mut socket: tokio::net::TcpStream,
    node_selector: Arc<RwLock<NodeSelector>>,
    _traffic: &TrafficCounter,
    config: &CognitiveProxyConfig,
    content_analyzer: ContentAnalyzer,
) -> Result<()> {
    // We already have the first byte as 0x05, now read the rest of the greeting
    let mut nmethods_byte = [0u8; 1];
    socket.read_exact(&mut nmethods_byte).await?;
    let nmethods = nmethods_byte[0] as usize;
    let mut methods = vec![0u8; nmethods];
    socket.read_exact(&mut methods).await?;

    // Reply: no auth required (method 0x00)
    socket.write_all(&[0x05, 0x00]).await?;

    // Read connect request: [ver, cmd, rsv, atyp, addr..., port]
    let mut req_header = [0u8; 4];
    socket.read_exact(&mut req_header).await?;
    if req_header[0] != 0x05 || req_header[1] != 0x01 {
        // Only support CONNECT (0x01)
        socket
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;
        anyhow::bail!("Unsupported SOCKS5 command: {}", req_header[1]);
    }

    let target = match req_header[3] {
        0x01 => {
            // IPv4
            let mut addr = [0u8; 4];
            socket.read_exact(&mut addr).await?;
            let mut port_bytes = [0u8; 2];
            socket.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            format!("{}.{}.{}.{}:{}", addr[0], addr[1], addr[2], addr[3], port)
        }
        0x03 => {
            // Domain name
            let mut len = [0u8; 1];
            socket.read_exact(&mut len).await?;
            let mut domain = vec![0u8; len[0] as usize];
            socket.read_exact(&mut domain).await?;
            let mut port_bytes = [0u8; 2];
            socket.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            format!("{}:{}", String::from_utf8_lossy(&domain), port)
        }
        0x04 => {
            // IPv6
            let mut addr = [0u8; 16];
            socket.read_exact(&mut addr).await?;
            let mut port_bytes = [0u8; 2];
            socket.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            let ipv6 = std::net::Ipv6Addr::from(addr);
            format!("[{ipv6}]:{port}")
        }
        _ => anyhow::bail!("Unknown SOCKS5 address type: {}", req_header[3]),
    };

    info!(%target, "Cognitive SOCKS5 CONNECT");

    // Intelligent node selection based on content analysis and priorities
    let selected_peer = select_intelligent_node(
        &node_selector,
        &content_analyzer,
        &target,
        b"", // No initial data to analyze yet
        config,
    ).await;

    if selected_peer.is_none() {
        anyhow::bail!("No suitable node available for request to {}", target);
    }

    let _provider_peer = selected_peer.unwrap();

    // Open P2P stream to selected node using the node selector
    // In a real implementation, this would use a global stream controller
    // For now, we'll need to have the control passed as a parameter or accessed globally
    anyhow::bail!("Cognitive proxy implementation requires additional stream controller integration");
}

/// Handle HTTP CONNECT with cognitive capabilities
async fn handle_cognitive_http_connect_from_start(
    mut socket: tokio::net::TcpStream,
    first_byte: u8,
    node_selector: Arc<RwLock<NodeSelector>>,
    _traffic: &TrafficCounter,
    config: &CognitiveProxyConfig,
    content_analyzer: ContentAnalyzer,
) -> Result<()> {
    // Parse HTTP CONNECT with timeout and size limits (Slowloris / OOM protection)
    let target = parse_http_connect(&mut socket, first_byte).await?;

    info!(%target, "Cognitive HTTP CONNECT");

    // Intelligent node selection based on content analysis and priorities
    let selected_peer = select_intelligent_node(
        &node_selector,
        &content_analyzer,
        &target,
        b"", // Headers no longer available after bounded parse
        config,
    ).await;

    if selected_peer.is_none() {
        anyhow::bail!("No suitable node available for request to {}", target);
    }

    let _provider_peer = selected_peer.unwrap();

    // Open P2P stream to selected node using the node selector
    // This requires integration with the swarm's stream control
    anyhow::bail!("Cognitive proxy implementation requires additional stream controller integration");
}

/// Intelligent node selection based on content analysis
async fn select_intelligent_node(
    node_selector: &Arc<RwLock<NodeSelector>>,
    content_analyzer: &ContentAnalyzer,
    target_url: &str,
    initial_data: &[u8],
    config: &CognitiveProxyConfig,
) -> Option<PeerId> {
    if !config.content_analysis_enabled {
        // Fall back to regular selection if content analysis is disabled
        let mut selector = node_selector.write();
        return selector.select_best();
    }

    // Analyze the target and initial data
    let content_analysis = content_analyzer.analyze_content(initial_data, target_url);

    // Boost AI-related requests if enabled
    if config.ai_priority_boost && content_analysis.ai_model_request {
        // Prioritize nodes that are better suited for AI workloads
        let mut selector = node_selector.write();

        // In a real implementation, we would adjust node scoring based on content_analysis
        // for AI-specific performance characteristics

        return selector.select_best();
    }

    // Regular selection
    let mut selector = node_selector.write();
    selector.select_best()
}