use anyhow::{Context, Result};
use libp2p::futures::AsyncWriteExt as FuturesAsyncWriteExt;
use libp2p::PeerId;
use libp2p_stream as stream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};

use super::PROXY_PROTOCOL;
use crate::traffic::TrafficCounter;

/// Start a local HTTP CONNECT proxy that tunnels traffic through a P2P stream to the provider node.
pub async fn start_http_proxy(
    port: u16,
    provider_peer: PeerId,
    control: stream::Control,
    traffic: TrafficCounter,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    info!(port, "HTTP CONNECT proxy listening");

    loop {
        let (socket, addr) = listener.accept().await?;
        let mut ctl = control.clone();
        let peer = provider_peer;
        let counter = traffic.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_http_connect(socket, peer, &mut ctl, &counter).await {
                warn!(%addr, "HTTP proxy error: {e:#}");
            }
        });
    }
}

/// Handle a single HTTP CONNECT request.
async fn handle_http_connect(
    socket: tokio::net::TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
) -> Result<()> {
    let mut reader = BufReader::new(socket);

    // Read "CONNECT host:port HTTP/1.1\r\n"
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 3 || parts[0] != "CONNECT" {
        let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
        reader.get_mut().write_all(response.as_bytes()).await?;
        anyhow::bail!("Invalid request: {request_line}");
    }

    let target = parts[1].to_string();

    // Skip remaining headers (read until empty line)
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if line.trim().is_empty() {
            break;
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
    let mut socket = reader.into_inner();
    socket
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    // Bidirectional relay between HTTP client and P2P stream
    traffic.inc_connections();
    let mut p2p_compat = p2p_stream.compat();
    if let Err(e) =
        crate::traffic::relay_bidirectional(&mut socket, &mut p2p_compat, Some(traffic)).await
    {
        warn!("socket relay failed: {e}");
    }
    traffic.dec_connections();

    Ok(())
}
