use std::sync::Arc;

use anyhow::{Context, Result};
use base64::Engine;
use libp2p::futures::AsyncWriteExt as FuturesAsyncWriteExt;
use libp2p::PeerId;
use libp2p_stream as stream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};

use super::{ProxyCredentials, PROXY_PROTOCOL};
use crate::traffic::TrafficCounter;

/// Starts a unified proxy server that handles both SOCKS5 and HTTP CONNECT protocols on a single port.
/// When `credentials` is Some, authentication is required for all connections.
pub async fn start_unified_proxy(
    port: u16,
    provider_peer: PeerId,
    control: stream::Control,
    traffic: TrafficCounter,
    credentials: Option<ProxyCredentials>,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    let creds = credentials.map(Arc::new);
    info!(port, auth = creds.is_some(), "Unified proxy listening on single port");

    loop {
        let (socket, addr) = listener.accept().await?;
        let mut ctl = control.clone();
        let peer = provider_peer;
        let counter = traffic.clone();
        let creds = creds.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_unified_connection(socket, peer, &mut ctl, &counter, creds.as_deref()).await {
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
    credentials: Option<&ProxyCredentials>,
) -> Result<()> {
    let mut first_byte_buffer = [0u8; 1];
    socket.read_exact(&mut first_byte_buffer).await?;

    let first_byte = first_byte_buffer[0];

    if first_byte == 0x05 {
        handle_socks5_from_start(socket, provider_peer, control, traffic, credentials).await
    } else {
        handle_http_connect_from_start(socket, first_byte, provider_peer, control, traffic, credentials).await
    }
}

/// Handle SOCKS5 from the beginning of the connection, having already read the first byte (0x05)
async fn handle_socks5_from_start(
    mut socket: tokio::net::TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
    credentials: Option<&ProxyCredentials>,
) -> Result<()> {
    // Read the rest of the greeting: nmethods + methods
    let mut nmethods_byte = [0u8; 1];
    socket.read_exact(&mut nmethods_byte).await?;
    let nmethods = nmethods_byte[0] as usize;
    let mut methods = vec![0u8; nmethods];
    socket.read_exact(&mut methods).await?;

    if let Some(creds) = credentials {
        // Require username/password auth (method 0x02)
        if !methods.contains(&0x02) {
            // Client doesn't support username/password auth — reject
            socket.write_all(&[0x05, 0xFF]).await?;
            anyhow::bail!("SOCKS5 client does not support username/password auth");
        }
        // Tell client to use username/password auth
        socket.write_all(&[0x05, 0x02]).await?;

        // RFC 1929: username/password sub-negotiation
        // Client sends: [ver(0x01), ulen, username..., plen, password...]
        let mut auth_ver = [0u8; 1];
        socket.read_exact(&mut auth_ver).await?;
        if auth_ver[0] != 0x01 {
            anyhow::bail!("Invalid SOCKS5 auth sub-negotiation version: {}", auth_ver[0]);
        }

        let mut ulen = [0u8; 1];
        socket.read_exact(&mut ulen).await?;
        let mut username = vec![0u8; ulen[0] as usize];
        socket.read_exact(&mut username).await?;

        let mut plen = [0u8; 1];
        socket.read_exact(&mut plen).await?;
        let mut password = vec![0u8; plen[0] as usize];
        socket.read_exact(&mut password).await?;

        let username_str = String::from_utf8_lossy(&username);
        let password_str = String::from_utf8_lossy(&password);

        if username_str != creds.username || password_str != creds.password {
            // Auth failure: [ver, status(0x01 = failure)]
            socket.write_all(&[0x01, 0x01]).await?;
            anyhow::bail!("SOCKS5 auth failed for user '{}'", username_str);
        }

        // Auth success: [ver, status(0x00 = success)]
        socket.write_all(&[0x01, 0x00]).await?;
    } else {
        // No auth required
        socket.write_all(&[0x05, 0x00]).await?;
    }

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

    let mut p2p_stream = control
        .open_stream(provider_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    p2p_stream
        .write_all(format!("{target}\n").as_bytes())
        .await?;
    p2p_stream.flush().await?;

    // Reply success
    socket
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    // Bidirectional relay
    traffic.inc_connections();
    let (mut client_read, mut client_write) = socket.into_split();
    let p2p_compat = p2p_stream.compat();
    let (mut p2p_read, mut p2p_write) = tokio::io::split(p2p_compat);

    let sent = &traffic.bytes_sent;
    let recv = &traffic.bytes_received;
    tokio::select! {
        r = crate::traffic::counted_copy(&mut client_read, &mut p2p_write, sent) => {
            if let Err(e) = r { warn!("client->p2p: {e}"); }
        }
        r = crate::traffic::counted_copy(&mut p2p_read, &mut client_write, recv) => {
            if let Err(e) = r { warn!("p2p->client: {e}"); }
        }
    }
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
    credentials: Option<&ProxyCredentials>,
) -> Result<()> {
    // Read the rest of the HTTP request line
    let mut request_line = Vec::new();
    request_line.push(first_byte);

    let mut prev_byte_was_cr = false;
    let mut byte = [0u8; 1];

    loop {
        socket.read_exact(&mut byte).await?;
        request_line.push(byte[0]);

        if prev_byte_was_cr && byte[0] == b'\n' {
            break;
        }
        prev_byte_was_cr = byte[0] == b'\r';
    }

    let request_line_str = String::from_utf8_lossy(&request_line);
    let parts: Vec<&str> = request_line_str.trim_end_matches("\r\n").trim().split_whitespace().collect();

    if parts.len() < 3 || parts[0] != "CONNECT" {
        let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
        socket.write_all(response.as_bytes()).await?;
        anyhow::bail!("Invalid request: {}", request_line_str);
    }

    let target = parts[1].to_string();

    // Read headers until \r\n\r\n
    let mut header_buffer = Vec::new();

    loop {
        socket.read_exact(&mut byte).await?;
        header_buffer.push(byte[0]);

        if header_buffer.len() >= 4 {
            let len = header_buffer.len();
            if header_buffer[len-4..] == [b'\r', b'\n', b'\r', b'\n'] {
                break;
            }
        }
    }

    // Authenticate if credentials are configured
    if let Some(creds) = credentials {
        let headers_str = String::from_utf8_lossy(&header_buffer);
        let authenticated = headers_str.lines().any(|line| {
            if let Some(value) = line.strip_prefix("Proxy-Authorization: Basic ") {
                let value = value.trim();
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(value) {
                    if let Ok(decoded_str) = String::from_utf8(decoded) {
                        let expected = format!("{}:{}", creds.username, creds.password);
                        return decoded_str == expected;
                    }
                }
            }
            false
        });

        if !authenticated {
            socket
                .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"nexlink\"\r\n\r\n")
                .await?;
            anyhow::bail!("HTTP CONNECT auth failed for target {}", target);
        }
    }

    info!(%target, "HTTP CONNECT");

    let mut p2p_stream = control
        .open_stream(provider_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    p2p_stream
        .write_all(format!("{target}\n").as_bytes())
        .await?;
    p2p_stream.flush().await?;

    socket
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    // Bidirectional relay
    traffic.inc_connections();
    let (mut client_read, mut client_write) = socket.into_split();
    let p2p_compat = p2p_stream.compat();
    let (mut p2p_read, mut p2p_write) = tokio::io::split(p2p_compat);

    let sent = &traffic.bytes_sent;
    let recv = &traffic.bytes_received;
    tokio::select! {
        r = crate::traffic::counted_copy(&mut client_read, &mut p2p_write, sent) => {
            if let Err(e) = r { warn!("client->p2p: {e}"); }
        }
        r = crate::traffic::counted_copy(&mut p2p_read, &mut client_write, recv) => {
            if let Err(e) = r { warn!("p2p->client: {e}"); }
        }
    }
    traffic.dec_connections();

    Ok(())
}
