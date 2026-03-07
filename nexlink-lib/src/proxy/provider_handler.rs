use anyhow::{Context, Result};
use libp2p::futures::AsyncReadExt as FuturesAsyncReadExt;
use libp2p::{PeerId, Stream};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use tokio::net::TcpStream;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::info;

use crate::traffic::TrafficCounter;

/// Maximum size for the target address line (4 KB) to prevent OOM from unbounded reads.
const MAX_TARGET_LINE_LEN: usize = 4096;

/// Handle an incoming proxy stream from a client node.
/// Protocol: first line is "host:port\n", then bidirectional raw bytes.
pub async fn handle_proxy_stream(
    peer_id: PeerId,
    stream: Stream,
    traffic: Option<&TrafficCounter>,
) -> Result<()> {
    info!(%peer_id, "Handling proxy stream");

    let mut reader = libp2p::futures::io::BufReader::new(stream);

    // Read target address (first line) with bounded size to prevent OOM
    let target_line = read_line_bounded(&mut reader, MAX_TARGET_LINE_LEN)
        .await
        .context("Failed to read target")?;
    let target = target_line.trim().to_string();

    if target.is_empty() {
        anyhow::bail!("Empty target address");
    }

    // Validate target against SSRF — reject private/internal addresses
    validate_target(&target)?;

    info!(%peer_id, %target, "Connecting to target");

    // Connect to target
    let mut tcp_stream = TcpStream::connect(&target)
        .await
        .with_context(|| format!("Failed to connect to {target}"))?;

    info!(%peer_id, %target, "Connected, starting relay");

    // Get buffered data and inner stream
    let buffered = reader.buffer().to_vec();
    let p2p_stream = reader.into_inner();

    // Convert libp2p stream (futures AsyncRead/Write) to tokio-compatible
    let mut p2p_compat = p2p_stream.compat();

    // Write any buffered data to TCP first
    if !buffered.is_empty() {
        tokio::io::AsyncWriteExt::write_all(&mut tcp_stream, &buffered).await?;
    }

    // Bidirectional relay with proper half-close
    if let Some(tc) = traffic {
        tc.inc_connections();
        crate::traffic::relay_bidirectional(
            &mut p2p_compat,
            &mut tcp_stream,
            &tc.bytes_received,
            &tc.bytes_sent,
        )
        .await;
        tc.dec_connections();
    } else {
        crate::traffic::relay_bidirectional_uncounted(&mut p2p_compat, &mut tcp_stream).await;
    }

    info!(%peer_id, %target, "Proxy stream ended");
    Ok(())
}

/// Read a line (up to `\n`) from a futures AsyncBufRead, enforcing a max byte limit.
/// Returns the line content (including the newline). Bails if the limit is exceeded
/// before a newline is found.
async fn read_line_bounded<R: libp2p::futures::AsyncBufReadExt + Unpin>(
    reader: &mut R,
    max_len: usize,
) -> Result<String> {
    let mut buf = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            break; // EOF
        }
        buf.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
        if buf.len() >= max_len {
            anyhow::bail!("Target address line exceeds {max_len} byte limit");
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Validate that a `host:port` target is not a private/internal address.
/// Rejects loopback, link-local, private RFC1918, cloud metadata, and IPv6 equivalents.
fn validate_target(target: &str) -> Result<()> {
    // Resolve the target to socket addresses so we catch DNS that points to internal IPs
    let addrs: Vec<_> = target
        .to_socket_addrs()
        .with_context(|| format!("Cannot resolve target: {target}"))?
        .collect();

    if addrs.is_empty() {
        anyhow::bail!("Target resolved to no addresses: {target}");
    }

    for addr in &addrs {
        if is_forbidden_ip(addr.ip()) {
            anyhow::bail!(
                "Target {target} resolves to forbidden address {}: private/internal IPs are blocked",
                addr.ip()
            );
        }
    }

    Ok(())
}

/// Returns true if the IP address is private, loopback, link-local, or otherwise
/// should not be reachable from a proxy provider.
fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_ipv4(v4),
        IpAddr::V6(v6) => is_forbidden_ipv6(v6),
    }
}

fn is_forbidden_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()                          // 127.0.0.0/8
        || ip.is_private()                    // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
        || ip.is_link_local()                 // 169.254.0.0/16
        || ip.is_broadcast()                  // 255.255.255.255
        || ip.is_unspecified()                // 0.0.0.0
        || ip.octets()[0] == 100 && ip.octets()[1] >= 64 && ip.octets()[1] <= 127  // CGNAT 100.64.0.0/10
        || ip == Ipv4Addr::new(169, 254, 169, 254) // AWS/cloud metadata
}

fn is_forbidden_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()                          // ::1
        || ip.is_unspecified()                // ::
        // IPv4-mapped IPv6 addresses (::ffff:x.x.x.x) — check the embedded v4
        || match ip.to_ipv4_mapped() {
            Some(v4) => is_forbidden_ipv4(v4),
            None => false,
        }
        // Unique local addresses (fc00::/7) — IPv6 equivalent of RFC1918
        || (ip.segments()[0] & 0xfe00) == 0xfc00
        // Link-local (fe80::/10)
        || (ip.segments()[0] & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forbidden_ips() {
        assert!(is_forbidden_ip("127.0.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("10.0.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("172.16.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("192.168.1.1".parse().unwrap()));
        assert!(is_forbidden_ip("169.254.169.254".parse().unwrap()));
        assert!(is_forbidden_ip("0.0.0.0".parse().unwrap()));
        assert!(is_forbidden_ip("::1".parse().unwrap()));
        assert!(is_forbidden_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("fe80::1".parse().unwrap()));
        assert!(is_forbidden_ip("fc00::1".parse().unwrap()));

        // Public IPs should be allowed
        assert!(!is_forbidden_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_forbidden_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_forbidden_ip("2606:4700::1111".parse().unwrap()));
    }
}
