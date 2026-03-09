use anyhow::{Context, Result};
use dashmap::DashMap;
use libp2p::futures::AsyncBufReadExt;
use libp2p::{PeerId, Stream};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};

use crate::traffic::TrafficCounter;

/// Handle an incoming proxy stream from a client node.
/// Protocol: "AUTH username password\n", then "host:port\n", then bidirectional raw bytes.
/// If `allowed_credentials` is provided, the AUTH line is verified against it.
pub async fn handle_proxy_stream(
    peer_id: PeerId,
    stream: Stream,
    traffic: Option<&TrafficCounter>,
    allowed_credentials: Option<&Arc<DashMap<String, String>>>,
) -> Result<()> {
    info!(%peer_id, "Handling proxy stream");

    let mut reader = libp2p::futures::io::BufReader::new(stream);

    // Read first line — could be AUTH or target
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .await
        .context("Failed to read first line")?;
    let first_line_trimmed = first_line.trim();

    let target = if let Some(rest) = first_line_trimmed.strip_prefix("AUTH ") {
        // Parse AUTH line: "AUTH <username> <password>"
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
            let username = parts[0];
            let password = parts[1];

            // Verify credentials if allowed_credentials is provided
            if let Some(creds_map) = allowed_credentials {
                match creds_map.get(username) {
                    Some(expected_pw) if expected_pw.value() == password => {
                        info!(%peer_id, %username, "AUTH verified");
                    }
                    _ => {
                        anyhow::bail!("AUTH failed for peer {peer_id}: invalid credentials");
                    }
                }
            } else {
                info!(%peer_id, username, "Received AUTH (no verification configured)");
            }
        } else {
            warn!(%peer_id, "Malformed AUTH line");
            if allowed_credentials.is_some() {
                anyhow::bail!("Malformed AUTH line from peer {peer_id}");
            }
        }
        // Read the next line as target
        let mut target_line = String::new();
        reader
            .read_line(&mut target_line)
            .await
            .context("Failed to read target after AUTH")?;
        target_line.trim().to_string()
    } else {
        // No AUTH prefix — reject if verification is required
        if allowed_credentials.is_some() {
            anyhow::bail!("No AUTH line from peer {peer_id}, credentials required");
        }
        first_line_trimmed.to_string()
    };

    if target.is_empty() {
        anyhow::bail!("Empty target address");
    }

    info!(%peer_id, %target, "Connecting to target");

    // Connect to target
    let tcp_stream = TcpStream::connect(&target)
        .await
        .with_context(|| format!("Failed to connect to {target}"))?;

    info!(%peer_id, %target, "Connected, starting relay");

    // Get buffered data and inner stream
    let buffered = reader.buffer().to_vec();
    let p2p_stream = reader.into_inner();

    // Convert libp2p stream (futures AsyncRead/Write) to tokio-compatible
    let mut p2p_compat = p2p_stream.compat();
    let mut tcp_stream = tcp_stream;

    // Write any buffered data to TCP first
    if !buffered.is_empty() {
        tokio::io::AsyncWriteExt::write_all(&mut tcp_stream, &buffered).await?;
    }

    // Bidirectional relay
    if let Some(tc) = traffic {
        tc.inc_connections();
    }

    if let Err(e) = crate::traffic::relay_bidirectional(&mut tcp_stream, &mut p2p_compat, traffic).await {
        warn!(%peer_id, "socket relay failed: {e}");
    }

    if let Some(tc) = traffic {
        tc.dec_connections();
    }

    info!(%peer_id, %target, "Proxy stream ended");
    Ok(())
}
