use anyhow::{Context, Result};
use libp2p::futures::AsyncBufReadExt;
use libp2p::{PeerId, Stream};
use tokio::net::TcpStream;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};

use crate::traffic::TrafficCounter;

/// Handle an incoming proxy stream from a client node.
/// Protocol: first line is "host:port\n", then bidirectional raw bytes.
pub async fn handle_proxy_stream(
    peer_id: PeerId,
    stream: Stream,
    traffic: Option<&TrafficCounter>,
) -> Result<()> {
    info!(%peer_id, "Handling proxy stream");

    let mut reader = libp2p::futures::io::BufReader::new(stream);

    // Read target address (first line)
    let mut target_line = String::new();
    reader
        .read_line(&mut target_line)
        .await
        .context("Failed to read target")?;
    let target = target_line.trim().to_string();

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
    let p2p_compat = p2p_stream.compat();
    let (mut p2p_read, mut p2p_write) = tokio::io::split(p2p_compat);
    let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();

    // Write any buffered data to TCP first
    if !buffered.is_empty() {
        tokio::io::AsyncWriteExt::write_all(&mut tcp_write, &buffered).await?;
    }

    // Bidirectional relay
    if let Some(tc) = traffic {
        tc.inc_connections();
    }

    if let Some(tc) = traffic {
        let recv = &tc.bytes_received;
        let sent = &tc.bytes_sent;
        tokio::select! {
            r = crate::traffic::counted_copy(&mut p2p_read, &mut tcp_write, recv) => {
                if let Err(e) = r { warn!(%peer_id, "client->target: {e}"); }
            }
            r = crate::traffic::counted_copy(&mut tcp_read, &mut p2p_write, sent) => {
                if let Err(e) = r { warn!(%peer_id, "target->client: {e}"); }
            }
        }
    } else {
        tokio::select! {
            r = tokio::io::copy(&mut p2p_read, &mut tcp_write) => {
                if let Err(e) = r { warn!(%peer_id, "client->target: {e}"); }
            }
            r = tokio::io::copy(&mut tcp_read, &mut p2p_write) => {
                if let Err(e) = r { warn!(%peer_id, "target->client: {e}"); }
            }
        }
    }

    if let Some(tc) = traffic {
        tc.dec_connections();
    }

    info!(%peer_id, %target, "Proxy stream ended");
    Ok(())
}
