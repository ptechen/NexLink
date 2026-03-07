use anyhow::{Context, Result};
use libp2p::futures::AsyncWriteExt as FuturesAsyncWriteExt;
use libp2p::PeerId;
use libp2p_stream as stream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};

use super::PROXY_PROTOCOL;
use crate::traffic::TrafficCounter;

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
    // Peek at the first byte to determine protocol type
    let mut first_byte_buffer = [0u8; 1];
    let bytes_read = socket.read_exact(&mut first_byte_buffer).await?;

    if bytes_read == 0 {
        anyhow::bail!("Empty connection received");
    }

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

    // Bidirectional relay between SOCKS5 client and P2P stream
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
    let parts: Vec<&str> = request_line_str.trim_end_matches("\r\n").trim().split_whitespace().collect();

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
            if header_buffer[len-4..] == [b'\r', b'\n', b'\r', b'\n'] {
                break;
            }
        }
    }

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

    // Bidirectional relay between HTTP client and P2P stream
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