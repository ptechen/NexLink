use anyhow::{Context, Result};
use libp2p::futures::AsyncWriteExt as FuturesAsyncWriteExt;
use libp2p::PeerId;
use libp2p_stream as stream;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};

use nexlink_traffic::{update_context, TrafficContextUpdate};

use super::{ProxyCredentials, PROXY_PROTOCOL};
use crate::pac::{add_proxy_rule, needs_proxy};
use crate::traffic::TrafficCounter;

/// Direct-connect timeout before falling back to P2P
const DIRECT_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Starts a unified proxy server that handles both SOCKS5 and HTTP CONNECT protocols on a single port.
/// No local authentication — only listens on 127.0.0.1.
/// Implements auto-learning split routing: known-blocked domains go via P2P, others try direct first.
pub async fn start_unified_proxy(
    port: u16,
    provider_peer: PeerId,
    control: stream::Control,
    traffic: TrafficCounter,
    credentials: ProxyCredentials,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    info!(port, "Unified proxy listening on single port");

    loop {
        let (socket, addr) = listener.accept().await?;
        let mut ctl = control.clone();
        let peer = provider_peer;
        let counter = traffic.clone();
        let creds = credentials.clone();

        tokio::spawn(async move {
            if let Err(e) =
                handle_unified_connection(socket, addr, peer, &mut ctl, &counter, &creds).await
            {
                warn!(%addr, "Unified proxy error: {e:#}");
            }
        });
    }
}

/// Handles a single incoming connection, detecting whether it's SOCKS5 or HTTP CONNECT
/// and routing to the appropriate handler.
async fn handle_unified_connection(
    mut socket: TcpStream,
    client_addr: std::net::SocketAddr,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
    credentials: &ProxyCredentials,
) -> Result<()> {
    let source_ip = client_addr.ip().to_string();
    update_context(
        provider_peer,
        TrafficContextUpdate {
            role: Some("client"),
            source: Some("nexlink-node"),
            source_ip: Some(source_ip.as_str()),
            source_transport: Some("tcp"),
        },
    );
    let mut first_byte_buffer = [0u8; 1];
    socket.read_exact(&mut first_byte_buffer).await?;

    let first_byte = first_byte_buffer[0];

    if first_byte == 0x05 {
        handle_socks5_from_start(socket, provider_peer, control, traffic, credentials).await
    } else {
        handle_http_connect_from_start(
            socket,
            first_byte,
            provider_peer,
            control,
            traffic,
            credentials,
        )
        .await
    }
}

/// Extract host from "host:port" target string
fn extract_host(target: &str) -> String {
    // Handle IPv6 like [::1]:443
    if target.starts_with('[') {
        if let Some(end) = target.find(']') {
            return target[1..end].to_string();
        }
    }
    // Normal host:port
    target
        .rsplit_once(':')
        .map(|(host, _)| host.to_string())
        .unwrap_or_else(|| target.to_string())
}

/// Route the connection: try direct first, fall back to P2P if needed
async fn route_connection(
    socket: &mut TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
    credentials: &ProxyCredentials,
    target: &str,
) -> Result<()> {
    let host = extract_host(target);

    if needs_proxy(&host) {
        info!(%target, "Routing via P2P (known proxy rule)");
        relay_via_p2p(socket, provider_peer, control, traffic, credentials, target).await
    } else {
        // Try direct connection with timeout
        match tokio::time::timeout(DIRECT_CONNECT_TIMEOUT, TcpStream::connect(target)).await {
            Ok(Ok(tcp_stream)) => {
                info!(%target, "Direct connection succeeded");
                relay_direct(socket, tcp_stream, traffic).await
            }
            Ok(Err(e)) => {
                info!(%target, error = %e, "Direct connection failed, falling back to P2P");
                add_proxy_rule(&host);
                relay_via_p2p(socket, provider_peer, control, traffic, credentials, target).await
            }
            Err(_) => {
                info!(%target, "Direct connection timed out, falling back to P2P");
                add_proxy_rule(&host);
                relay_via_p2p(socket, provider_peer, control, traffic, credentials, target).await
            }
        }
    }
}

/// Relay traffic via P2P stream to provider
async fn relay_via_p2p(
    socket: &mut TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
    credentials: &ProxyCredentials,
    target: &str,
) -> Result<()> {
    let mut p2p_stream = control
        .open_stream(provider_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    p2p_stream
        .write_all(format!("AUTH {} {}\n", credentials.username, credentials.password).as_bytes())
        .await?;
    p2p_stream
        .write_all(format!("{target}\n").as_bytes())
        .await?;
    p2p_stream.flush().await?;

    traffic.inc_connections();
    let mut p2p_compat = p2p_stream.compat();
    if let Err(e) =
        crate::traffic::relay_bidirectional(socket, &mut p2p_compat, Some(traffic)).await
    {
        warn!("P2P relay failed: {e}");
    }
    traffic.dec_connections();

    Ok(())
}

/// Relay traffic directly between client and target (no P2P)
async fn relay_direct(
    socket: &mut TcpStream,
    mut tcp_stream: TcpStream,
    traffic: &TrafficCounter,
) -> Result<()> {
    traffic.inc_connections();
    if let Err(e) =
        crate::traffic::relay_bidirectional(socket, &mut tcp_stream, Some(traffic)).await
    {
        warn!("Direct relay failed: {e}");
    }
    traffic.dec_connections();

    Ok(())
}

/// Handle SOCKS5 from the beginning of the connection, having already read the first byte (0x05)
async fn handle_socks5_from_start(
    mut socket: TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
    credentials: &ProxyCredentials,
) -> Result<()> {
    // Read the rest of the greeting: nmethods + methods
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
        socket
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;
        anyhow::bail!("Unsupported SOCKS5 command: {}", req_header[1]);
    }

    let target = match req_header[3] {
        0x01 => {
            let mut addr = [0u8; 4];
            socket.read_exact(&mut addr).await?;
            let mut port_bytes = [0u8; 2];
            socket.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            format!("{}.{}.{}.{}:{}", addr[0], addr[1], addr[2], addr[3], port)
        }
        0x03 => {
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

    // Reply success to SOCKS5 client before routing
    socket
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    // Route: direct or P2P
    route_connection(&mut socket, provider_peer, control, traffic, credentials, &target).await
}

/// Handle HTTP CONNECT from the beginning of the connection, with the first byte already read
async fn handle_http_connect_from_start(
    mut socket: TcpStream,
    first_byte: u8,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
    credentials: &ProxyCredentials,
) -> Result<()> {
    // We need to read the rest of the HTTP request
    let mut request_line = Vec::new();
    request_line.push(first_byte);

    // Read until we hit the end of the request line (\r\n)
    let mut prev_byte_was_cr = false;
    let mut byte = [0u8; 1];

    loop {
        socket.read_exact(&mut byte).await?;
        request_line.push(byte[0]);

        if prev_byte_was_cr && byte[0] == b'\n' {
            break; // Found \r\n
        }
        prev_byte_was_cr = byte[0] == b'\r';
    }

    // Parse the request line
    let request_line_str = String::from_utf8_lossy(&request_line);
    let parts: Vec<&str> = request_line_str
        .trim_end_matches("\r\n")
        .trim()
        .split_whitespace()
        .collect();

    if parts.len() < 3 || parts[0] != "CONNECT" {
        let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
        socket.write_all(response.as_bytes()).await?;
        anyhow::bail!("Invalid request: {}", request_line_str);
    }

    let target = parts[1].to_string();

    // Now read the headers until we get \r\n\r\n (empty line)
    let mut header_buffer = Vec::new();

    loop {
        socket.read_exact(&mut byte).await?;
        header_buffer.push(byte[0]);

        // Check for \r\n\r\n pattern (end of headers)
        if header_buffer.len() >= 4 {
            let len = header_buffer.len();
            if header_buffer[len - 4..] == [b'\r', b'\n', b'\r', b'\n'] {
                break;
            }
        }
    }

    info!(%target, "HTTP CONNECT");

    // Reply 200 Connection Established to client before routing
    socket
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    // Route: direct or P2P
    route_connection(&mut socket, provider_peer, control, traffic, credentials, &target).await
}
