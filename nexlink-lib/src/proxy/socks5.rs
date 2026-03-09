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

/// Start a local SOCKS5 proxy that tunnels traffic through a P2P stream to the provider node.
pub async fn start_socks5_proxy(
    port: u16,
    provider_peer: PeerId,
    control: stream::Control,
    traffic: TrafficCounter,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    info!(port, "SOCKS5 proxy listening");

    loop {
        let (socket, addr) = listener.accept().await?;
        let mut ctl = control.clone();
        let peer = provider_peer;
        let counter = traffic.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_socks5(socket, peer, &mut ctl, &counter).await {
                warn!(%addr, "SOCKS5 error: {e:#}");
            }
        });
    }
}

/// Handle a single SOCKS5 connection (CONNECT command only).
async fn handle_socks5(
    mut socket: tokio::net::TcpStream,
    provider_peer: PeerId,
    control: &mut stream::Control,
    traffic: &TrafficCounter,
) -> Result<()> {
    // === SOCKS5 Handshake ===

    // 1. Read greeting: [version, nmethods, methods...]
    let mut header = [0u8; 2];
    socket.read_exact(&mut header).await?;
    if header[0] != 0x05 {
        anyhow::bail!("Not SOCKS5");
    }
    let nmethods = header[1] as usize;
    let mut methods = vec![0u8; nmethods];
    socket.read_exact(&mut methods).await?;

    // 2. Reply: no auth required (method 0x00)
    socket.write_all(&[0x05, 0x00]).await?;

    // 3. Read connect request: [ver, cmd, rsv, atyp, addr..., port]
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

    // 4. Open P2P stream to provider node
    let mut p2p_stream = control
        .open_stream(provider_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    // Send target as first line of P2P protocol
    p2p_stream
        .write_all(format!("{target}\n").as_bytes())
        .await?;
    p2p_stream.flush().await?;

    // 5. Reply success to SOCKS5 client
    //    [ver, rep(success), rsv, atyp(ipv4), bind_addr(0.0.0.0), bind_port(0)]
    socket
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    // 6. Bidirectional relay between SOCKS5 client and P2P stream
    traffic.inc_connections();
    let mut p2p_compat = p2p_stream.compat();
    if let Err(e) = crate::traffic::relay_bidirectional(&mut socket, &mut p2p_compat, Some(traffic)).await {
        warn!("socket relay failed: {e}");
    }
    traffic.dec_connections();

    Ok(())
}
