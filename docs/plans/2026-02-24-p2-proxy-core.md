# P2: Proxy Core Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement SOCKS5/HTTP local proxy and P2P stream forwarding, so traffic flows: user app → local proxy → P2P stream → exit node → target.

**Architecture:** Local SOCKS5/HTTP proxy accepts connections, opens libp2p streams to exit node, sends target address as first line, then bidirectional raw byte relay. Exit node reads target, connects TCP, relays bytes. Uses `libp2p-stream` crate for custom protocol streams and `tokio-util::compat` to bridge tokio/futures AsyncRead/Write.

**Tech Stack:** libp2p-stream 0.4.0-alpha, fast-socks5 0.10, tokio-util (compat), hyper (HTTP CONNECT)

---

### Task 1: Add Dependencies and stream::Behaviour to ClashBehaviour

**Files:**
- Modify: `Cargo.toml` (workspace root — add new workspace deps)
- Modify: `clash-lib/Cargo.toml` (add libp2p-stream, tokio-util, fast-socks5)
- Modify: `clash-lib/src/network/behaviour.rs` (add stream::Behaviour)
- Modify: `clash-lib/src/network/swarm.rs` (update swarm builders)

**Step 1: Add workspace dependencies**

In root `Cargo.toml`, add to `[workspace.dependencies]`:

```toml
libp2p-stream = "0.4.0-alpha"
tokio-util = { version = "0.7", features = ["compat"] }
fast-socks5 = "0.10"
```

NOTE: `libp2p-stream` version must be compatible with libp2p 0.56. If `0.4.0-alpha` doesn't compile, try `0.3` or `0.2`. Check the error and adjust.

**Step 2: Add to clash-lib/Cargo.toml**

```toml
libp2p-stream = { workspace = true }
tokio-util = { workspace = true }
fast-socks5 = { workspace = true }
```

**Step 3: Add stream::Behaviour to ClashBehaviour**

In `clash-lib/src/network/behaviour.rs`:

```rust
use libp2p::swarm::NetworkBehaviour;
use libp2p::{identify, ping, relay, rendezvous};
use libp2p_stream as stream;

#[derive(NetworkBehaviour)]
pub struct ClashBehaviour {
    pub relay_client: relay::client::Behaviour,
    pub identify: identify::Behaviour,
    pub rendezvous_client: rendezvous::client::Behaviour,
    pub ping: ping::Behaviour,
    pub stream: stream::Behaviour,
}
```

**Step 4: Update build_client_swarm**

In `clash-lib/src/network/swarm.rs`, add `stream: stream::Behaviour::new()` to the ClashBehaviour initialization in `build_client_swarm`.

**Step 5: Verify compilation**

Run: `cargo build -p clash-lib`

If `libp2p-stream` version conflicts, try different versions. The key constraint is that `libp2p-stream` depends on `libp2p-swarm` and the version must match what `libp2p 0.56` pulls in.

**Step 6: Commit**

```bash
git commit -m "feat: add libp2p-stream, fast-socks5, tokio-util dependencies"
```

---

### Task 2: Implement Proxy Module — Exit Node Handler

**Files:**
- Create: `clash-lib/src/proxy/mod.rs`
- Create: `clash-lib/src/proxy/exit_handler.rs`
- Modify: `clash-lib/src/lib.rs` (add `pub mod proxy;`)

The exit handler accepts incoming P2P streams, reads the target address, connects to the target via TCP, and relays bytes bidirectionally.

**Step 1: Create proxy/mod.rs**

```rust
pub mod exit_handler;
pub mod socks5;
pub mod http_connect;

use libp2p::StreamProtocol;

pub const PROXY_PROTOCOL: StreamProtocol = StreamProtocol::new("/clash/proxy/1.0.0");
```

**Step 2: Implement exit_handler.rs**

```rust
use anyhow::{Context, Result};
use futures::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use libp2p::{PeerId, Stream};
use tokio::net::TcpStream;
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use tracing::{info, warn};

/// Handle an incoming proxy stream from a client node.
/// Protocol: first line is "host:port\n", then bidirectional raw bytes.
pub async fn handle_proxy_stream(peer_id: PeerId, stream: Stream) -> Result<()> {
    info!(%peer_id, "Handling proxy stream");

    // Wrap the libp2p stream (futures AsyncRead/Write) for buffered reading
    let mut reader = futures::io::BufReader::new(stream);

    // Read the target address (first line: "host:port\n")
    let mut target_line = String::new();
    reader
        .read_line(&mut target_line)
        .await
        .context("Failed to read target address")?;
    let target = target_line.trim();

    if target.is_empty() {
        anyhow::bail!("Empty target address");
    }

    info!(%peer_id, %target, "Connecting to target");

    // Connect to the target via TCP
    let tcp_stream = TcpStream::connect(target)
        .await
        .with_context(|| format!("Failed to connect to {target}"))?;

    info!(%peer_id, %target, "Connected to target, starting relay");

    // Get the inner stream back from BufReader
    // We need to handle any buffered data that was read ahead
    let (p2p_stream, buffered) = {
        let buf = reader.buffer().to_vec();
        (reader.into_inner(), buf)
    };

    // Bridge: libp2p Stream (futures) <-> TcpStream (tokio)
    // Convert libp2p stream to tokio-compatible
    let p2p_compat = p2p_stream.compat();
    let (mut p2p_read, mut p2p_write) = tokio::io::split(p2p_compat);

    let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();

    // Write any buffered data to TCP first
    if !buffered.is_empty() {
        tokio::io::AsyncWriteExt::write_all(&mut tcp_write, &buffered).await?;
    }

    // Bidirectional relay
    let client_to_target = tokio::io::copy(&mut p2p_read, &mut tcp_write);
    let target_to_client = tokio::io::copy(&mut tcp_read, &mut p2p_write);

    tokio::select! {
        r = client_to_target => {
            if let Err(e) = r { warn!(%peer_id, "client→target error: {e}"); }
        }
        r = target_to_client => {
            if let Err(e) = r { warn!(%peer_id, "target→client error: {e}"); }
        }
    }

    info!(%peer_id, %target, "Proxy stream ended");
    Ok(())
}
```

IMPORTANT NOTES:
- `libp2p::Stream` uses `futures::AsyncRead/AsyncWrite`. To use with tokio's `copy`, convert via `tokio_util::compat::FuturesAsyncReadCompatExt::compat()`.
- The `.compat()` method converts futures traits to tokio traits.
- BufReader may read ahead past the newline. Anything buffered beyond the first line must be forwarded to the TCP connection.
- If the BufReader API makes it hard to extract buffered data, an alternative is to manually read byte-by-byte until `\n`, then use the raw stream directly.

**Step 3: Add `pub mod proxy;` to lib.rs**

**Step 4: Verify compilation**

Run: `cargo build -p clash-lib`

**Step 5: Commit**

```bash
git commit -m "feat(proxy): implement exit node stream handler"
```

---

### Task 3: Implement SOCKS5 Local Proxy

**Files:**
- Create: `clash-lib/src/proxy/socks5.rs`

The SOCKS5 proxy listens on a local port, handles SOCKS5 handshake, opens a P2P stream to the exit node, sends the target address, and relays bytes.

**Step 1: Implement socks5.rs**

```rust
use anyhow::{Context, Result};
use fast_socks5::server::Socks5ServerProtocol;
use fast_socks5::util::target_addr::TargetAddr;
use fast_socks5::{ReplyError, Socks5Command};
use futures::AsyncWriteExt as FuturesAsyncWriteExt;
use libp2p::PeerId;
use libp2p_stream as stream;
use tokio::net::TcpListener;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tracing::{info, warn};

use super::PROXY_PROTOCOL;

/// Start SOCKS5 proxy server on the given port.
/// Proxies traffic through the P2P network to the specified exit node.
pub async fn start_socks5_proxy(
    port: u16,
    exit_peer: PeerId,
    control: stream::Control,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    info!(port, "SOCKS5 proxy listening");

    loop {
        let (socket, addr) = listener.accept().await?;
        let mut ctl = control.clone();
        let peer = exit_peer;

        tokio::spawn(async move {
            if let Err(e) = handle_socks5_connection(socket, peer, &mut ctl).await {
                warn!(%addr, "SOCKS5 error: {e:#}");
            }
        });
    }
}

async fn handle_socks5_connection(
    socket: tokio::net::TcpStream,
    exit_peer: PeerId,
    control: &mut stream::Control,
) -> Result<()> {
    // SOCKS5 handshake — no authentication
    let authed = Socks5ServerProtocol::accept_no_auth(socket)
        .await
        .context("SOCKS5 auth failed")?;

    // Read command and target address (keep domain unresolved — let exit node resolve)
    let (proto, cmd, target_addr) = authed
        .read_command()
        .await
        .context("SOCKS5 read command failed")?;

    match cmd {
        Socks5Command::TCPConnect => {}
        _ => {
            let _ = proto.reply_error(&ReplyError::CommandNotSupported).await;
            anyhow::bail!("Unsupported SOCKS5 command: {cmd:?}");
        }
    }

    // Format target as "host:port"
    let target_str = match &target_addr {
        TargetAddr::Ip(addr) => addr.to_string(),
        TargetAddr::Domain(domain, port) => format!("{domain}:{port}"),
    };

    info!(%target_str, "SOCKS5 CONNECT");

    // Open P2P stream to exit node
    let mut p2p_stream = control
        .open_stream(exit_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    // Send target address as first line
    p2p_stream
        .write_all(format!("{target_str}\n").as_bytes())
        .await
        .context("Failed to send target to exit node")?;
    p2p_stream.flush().await?;

    // Reply success to SOCKS5 client — this returns the raw TcpStream
    let bind_addr = "0.0.0.0:0".parse().unwrap();
    let client_socket = proto
        .reply_success(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("SOCKS5 reply error: {e}"))?;

    // Bridge: TcpStream (tokio) <-> P2P Stream (futures)
    let p2p_compat = tokio_util::compat::FuturesAsyncReadCompatExt::compat(p2p_stream);
    let (mut p2p_read, mut p2p_write) = tokio::io::split(p2p_compat);
    let (mut client_read, mut client_write) = tokio::io::split(client_socket);

    // Bidirectional relay
    tokio::select! {
        r = tokio::io::copy(&mut client_read, &mut p2p_write) => {
            if let Err(e) = r { warn!("client→p2p error: {e}"); }
        }
        r = tokio::io::copy(&mut p2p_read, &mut client_write) => {
            if let Err(e) = r { warn!("p2p→client error: {e}"); }
        }
    }

    Ok(())
}
```

IMPORTANT NOTES:
- `fast-socks5` 0.10 API may differ from 1.0.0-rc.0. Check the actual method names:
  - It might be `Socks5ServerProtocol::accept_no_auth()` or `Socks5Socket::new()`
  - The typestate API may not exist in 0.10 — it may use a different pattern
  - If 0.10 API is too different, consider using 1.0.0-rc.0 or implementing minimal SOCKS5 handshake manually
- Target address is NOT resolved — domain names are passed to the exit node for resolution (avoids DNS leaks)
- `fast_socks5::ReplyError` and `Socks5Command` import paths may differ — check actual crate exports

**Step 2: Verify compilation**

Run: `cargo build -p clash-lib`

**Step 3: Commit**

```bash
git commit -m "feat(proxy): implement SOCKS5 local proxy with P2P tunneling"
```

---

### Task 4: Implement HTTP CONNECT Proxy

**Files:**
- Create: `clash-lib/src/proxy/http_connect.rs`

A minimal HTTP CONNECT proxy. Simpler than full HTTP proxy — only handles CONNECT method.

**Step 1: Implement http_connect.rs**

```rust
use anyhow::{Context, Result};
use futures::AsyncWriteExt as FuturesAsyncWriteExt;
use libp2p::PeerId;
use libp2p_stream as stream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::{info, warn};

use super::PROXY_PROTOCOL;

/// Start HTTP CONNECT proxy server on the given port.
pub async fn start_http_proxy(
    port: u16,
    exit_peer: PeerId,
    control: stream::Control,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    info!(port, "HTTP CONNECT proxy listening");

    loop {
        let (socket, addr) = listener.accept().await?;
        let mut ctl = control.clone();
        let peer = exit_peer;

        tokio::spawn(async move {
            if let Err(e) = handle_http_connect(socket, peer, &mut ctl).await {
                warn!(%addr, "HTTP proxy error: {e:#}");
            }
        });
    }
}

async fn handle_http_connect(
    socket: tokio::net::TcpStream,
    exit_peer: PeerId,
    control: &mut stream::Control,
) -> Result<()> {
    let mut reader = BufReader::new(socket);

    // Read the first line: "CONNECT host:port HTTP/1.1\r\n"
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 3 || parts[0] != "CONNECT" {
        let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
        reader.get_mut().write_all(response.as_bytes()).await?;
        anyhow::bail!("Invalid CONNECT request: {request_line}");
    }

    let target = parts[1].to_string();

    // Read and discard remaining headers until empty line
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).await?;
        if header.trim().is_empty() {
            break;
        }
    }

    info!(%target, "HTTP CONNECT");

    // Open P2P stream to exit node
    let mut p2p_stream = control
        .open_stream(exit_peer, PROXY_PROTOCOL)
        .await
        .context("Failed to open P2P stream")?;

    // Send target address
    p2p_stream
        .write_all(format!("{target}\n").as_bytes())
        .await?;
    p2p_stream.flush().await?;

    // Reply 200 to client
    let mut socket = reader.into_inner();
    socket
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    // Bridge: TcpStream (tokio) <-> P2P Stream (futures)
    let p2p_compat = tokio_util::compat::FuturesAsyncReadCompatExt::compat(p2p_stream);
    let (mut p2p_read, mut p2p_write) = tokio::io::split(p2p_compat);
    let (mut client_read, mut client_write) = tokio::io::split(socket);

    tokio::select! {
        r = tokio::io::copy(&mut client_read, &mut p2p_write) => {
            if let Err(e) = r { warn!("client→p2p error: {e}"); }
        }
        r = tokio::io::copy(&mut p2p_read, &mut client_write) => {
            if let Err(e) = r { warn!("p2p→client error: {e}"); }
        }
    }

    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cargo build -p clash-lib`

**Step 3: Commit**

```bash
git commit -m "feat(proxy): implement HTTP CONNECT proxy with P2P tunneling"
```

---

### Task 5: Wire Proxy into clash-node

**Files:**
- Modify: `clash-node/src/main.rs`
- Modify: `clash-node/Cargo.toml` (add libp2p-stream dep)

The node binary needs to:
- If `--exit-node`: accept incoming P2P proxy streams and handle them
- Otherwise (client mode): start SOCKS5 + HTTP proxy listeners that tunnel through P2P

**Step 1: Add deps to clash-node**

```toml
libp2p-stream = { workspace = true }
```

**Step 2: Update main.rs**

After building the swarm, get a stream Control handle. Based on `--exit-node` flag:

**Exit node mode:**
```rust
// Accept incoming proxy streams
let mut incoming_control = swarm.behaviour().stream.new_control();
let mut incoming = incoming_control
    .accept(clash_lib::proxy::PROXY_PROTOCOL)
    .expect("protocol not registered");

// Spawn handler task
tokio::spawn(async move {
    use futures::StreamExt;
    while let Some((peer_id, stream)) = incoming.next().await {
        tokio::spawn(async move {
            if let Err(e) = clash_lib::proxy::exit_handler::handle_proxy_stream(peer_id, stream).await {
                warn!(%peer_id, "Proxy stream error: {e:#}");
            }
        });
    }
});
```

**Client mode:**
After discovering an exit node peer:
```rust
let proxy_control = swarm.behaviour().stream.new_control();

// Start SOCKS5 proxy
let socks_ctl = proxy_control.clone();
tokio::spawn(async move {
    if let Err(e) = clash_lib::proxy::socks5::start_socks5_proxy(1080, exit_peer, socks_ctl).await {
        warn!("SOCKS5 proxy error: {e:#}");
    }
});

// Start HTTP proxy
let http_ctl = proxy_control.clone();
tokio::spawn(async move {
    if let Err(e) = clash_lib::proxy::http_connect::start_http_proxy(8080, exit_peer, http_ctl).await {
        warn!("HTTP proxy error: {e:#}");
    }
});
```

The tricky part is that the client needs to know which peer to use as exit node. For now, use the first discovered peer that is not the relay. Later this will be selectable via UI.

Add `--socks5-port` and `--http-port` CLI args (default 1080 and 8080).

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git commit -m "feat(node): wire proxy into node binary with exit/client modes"
```

---

### Task 6: Integration Test — Proxy Traffic Through P2P

**Step 1: Start relay**

```bash
RUST_LOG=info cargo run -p clash-relay
```

Note the relay address.

**Step 2: Start exit node**

```bash
RUST_LOG=info cargo run -p clash-node -- --relay <RELAY_ADDR> --exit-node --data-dir ~/.clash/exit_a
```

**Step 3: Start client node**

```bash
RUST_LOG=info cargo run -p clash-node -- --relay <RELAY_ADDR> --data-dir ~/.clash/client_a
```

Wait for "Discovered peer" and "SOCKS5 proxy listening on port 1080".

**Step 4: Test SOCKS5 proxy**

```bash
curl -x socks5://127.0.0.1:1080 http://httpbin.org/ip
```

Expected: Returns your exit node's public IP.

**Step 5: Test HTTP CONNECT proxy**

```bash
curl -x http://127.0.0.1:8080 https://httpbin.org/ip
```

Expected: Returns your exit node's public IP (HTTPS via CONNECT tunnel).

**Step 6: Commit milestone**

```bash
git commit -m "milestone: P2 complete - traffic proxied through P2P network"
```

---

## Notes

- **tokio/futures bridge**: libp2p uses `futures` crate traits, tokio uses its own. Use `tokio_util::compat` to convert between them. `.compat()` on a futures reader gives a tokio reader, and vice versa.
- **fast-socks5 API**: If 0.10 API differs significantly from the plan, either adapt or implement a minimal SOCKS5 handshake manually (it's only ~30 lines for CONNECT-only support).
- **Exit node selection**: For P2 we use the first discovered peer. P5 will add proper exit node selection.
- **Error handling**: Proxy stream errors should not crash the node. Each connection is handled in its own task with error logging.
