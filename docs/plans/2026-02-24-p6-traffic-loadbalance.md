# P6: Traffic Statistics & Load Balancing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add real traffic statistics, multi-exit node latency-priority load balancing, node scoring, and config persistence.

**Architecture:** Atomic counters in proxy layer for traffic counting, Ping-based latency measurement for node scoring, auto-selection of lowest-latency exit node, and persistence of selected node in existing network.json.

**Tech Stack:** libp2p Ping protocol, Arc<AtomicU64>/AtomicU32, tokio interval sampling

---

### Task 1: TrafficCounter shared structure

**Files:**
- Create: `clash-lib/src/traffic.rs`
- Modify: `clash-lib/src/lib.rs` — add `pub mod traffic`

**Step 1: Create TrafficCounter**

```rust
// clash-lib/src/traffic.rs
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct TrafficCounter {
    pub bytes_sent: Arc<AtomicU64>,
    pub bytes_received: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU32>,
}

impl TrafficCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_sent(&self, n: u64) {
        self.bytes_sent.fetch_add(n, Ordering::Relaxed);
    }

    pub fn add_received(&self, n: u64) {
        self.bytes_received.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TrafficSnapshot {
        TrafficSnapshot {
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
        }
    }
}

pub struct TrafficSnapshot {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: u32,
}
```

**Step 2: Register module**

Add `pub mod traffic;` to `clash-lib/src/lib.rs`.

**Step 3: Build verification**

Run: `cargo build --workspace`

**Step 4: Commit**

```bash
git add clash-lib/src/traffic.rs clash-lib/src/lib.rs
git commit -m "feat(traffic): add TrafficCounter with atomic counters"
```

---

### Task 2: Instrument proxy handlers with traffic counting

**Files:**
- Modify: `clash-lib/src/proxy/socks5.rs` — accept TrafficCounter, count bytes
- Modify: `clash-lib/src/proxy/http_connect.rs` — accept TrafficCounter, count bytes
- Modify: `clash-lib/src/proxy/exit_handler.rs` — accept TrafficCounter, count bytes

**Approach:** Replace `tokio::io::copy(&mut r, &mut w)` with a counting copy helper, or wrap streams with a counting adapter. The simplest approach:

```rust
// In traffic.rs, add a counting copy function:
pub async fn counted_copy<R, W>(
    reader: &mut R,
    writer: &mut W,
    counter: &AtomicU64,
) -> tokio::io::Result<u64>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = tokio::io::AsyncReadExt::read(reader, &mut buf).await?;
        if n == 0 { break; }
        tokio::io::AsyncWriteExt::write_all(writer, &buf[..n]).await?;
        counter.fetch_add(n as u64, Ordering::Relaxed);
        total += n as u64;
    }
    Ok(total)
}
```

**Step 1: Add counted_copy to traffic.rs**

**Step 2: Update socks5.rs**
- Change `start_socks5_proxy` signature to accept `TrafficCounter`
- In the per-connection handler, call `counter.inc_connections()` at start, `counter.dec_connections()` at end
- Replace `tokio::io::copy` with `counted_copy` using `counter.bytes_sent` / `counter.bytes_received`

**Step 3: Update http_connect.rs** — same pattern as socks5

**Step 4: Update exit_handler.rs** — same pattern (counts from exit node perspective)

**Step 5: Build verification**

Run: `cargo build --workspace`

**Step 6: Commit**

```bash
git add clash-lib/src/traffic.rs clash-lib/src/proxy/
git commit -m "feat(traffic): instrument proxy handlers with byte counting"
```

---

### Task 3: NodeScore and NodeSelector

**Files:**
- Create: `clash-lib/src/node_score.rs`
- Modify: `clash-lib/src/lib.rs` — add `pub mod node_score`

**Step 1: Create node_score module**

```rust
// clash-lib/src/node_score.rs
use libp2p::PeerId;
use std::collections::HashMap;
use std::time::Instant;

pub struct NodeScore {
    pub latency_ms: Option<u64>,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_seen: Instant,
    pub connected: bool,
}

impl NodeScore {
    pub fn new() -> Self {
        Self {
            latency_ms: None,
            success_count: 0,
            failure_count: 0,
            last_seen: Instant::now(),
            connected: false,
        }
    }

    pub fn score(&self) -> f64 {
        let latency_score = match self.latency_ms {
            Some(ms) => 1000.0 / (ms as f64 + 1.0),
            None => 0.0, // Unknown latency = lowest priority
        };
        let total = self.success_count + self.failure_count;
        let success_ratio = if total > 0 {
            self.success_count as f64 / total as f64
        } else {
            0.5
        };
        let stale_penalty = if self.last_seen.elapsed().as_secs() > 60 { 50.0 } else { 0.0 };
        latency_score + success_ratio * 100.0 - stale_penalty
    }
}

pub struct NodeSelector {
    scores: HashMap<PeerId, NodeScore>,
    current: Option<PeerId>,
}

impl NodeSelector {
    pub fn new() -> Self {
        Self { scores: HashMap::new(), current: None }
    }

    pub fn update_latency(&mut self, peer: PeerId, rtt_ms: u64) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.latency_ms = Some(rtt_ms);
        entry.last_seen = Instant::now();
    }

    pub fn record_success(&mut self, peer: PeerId) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.success_count += 1;
        entry.last_seen = Instant::now();
    }

    pub fn record_failure(&mut self, peer: PeerId) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.failure_count += 1;
    }

    pub fn set_connected(&mut self, peer: PeerId, connected: bool) {
        let entry = self.scores.entry(peer).or_insert_with(NodeScore::new);
        entry.connected = connected;
        entry.last_seen = Instant::now();
    }

    pub fn remove_peer(&mut self, peer: &PeerId) {
        self.scores.remove(peer);
        if self.current.as_ref() == Some(peer) {
            self.current = None;
        }
    }

    /// Select the best connected node. Returns Some(peer) if changed, None if unchanged.
    pub fn select_best(&mut self) -> Option<PeerId> {
        let best = self.scores.iter()
            .filter(|(_, s)| s.connected)
            .max_by(|(_, a), (_, b)| a.score().partial_cmp(&b.score()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(p, _)| *p);

        if best != self.current {
            self.current = best;
            best
        } else {
            None // No change
        }
    }

    pub fn current(&self) -> Option<PeerId> {
        self.current
    }

    pub fn set_current(&mut self, peer: Option<PeerId>) {
        self.current = peer;
    }

    /// Get peer info for frontend display
    pub fn peer_scores(&self) -> Vec<(PeerId, Option<u64>, bool)> {
        self.scores.iter()
            .filter(|(_, s)| s.connected)
            .map(|(p, s)| (*p, s.latency_ms, self.current == Some(*p)))
            .collect()
    }
}
```

**Step 2: Register module in lib.rs**

**Step 3: Build verification**

**Step 4: Commit**

```bash
git add clash-lib/src/node_score.rs clash-lib/src/lib.rs
git commit -m "feat(score): add NodeScore and NodeSelector for latency-priority selection"
```

---

### Task 4: Integrate TrafficCounter + NodeSelector into swarm_task

**Files:**
- Modify: `clash-app/src-tauri/src/swarm_task.rs`

**Step 1: Initialize TrafficCounter and NodeSelector at startup**

- Create `TrafficCounter::new()` and `NodeSelector::new()`
- Pass TrafficCounter clone to proxy spawn calls (socks5, http_connect)
- Load `last_exit_node` from NetworkConfig if available

**Step 2: Handle Ping events for latency updates**

```rust
SwarmEvent::Behaviour(ClashBehaviourEvent::Ping(ping::Event { peer, result, .. })) => {
    match result {
        Ok(rtt) => {
            node_selector.update_latency(peer, rtt.as_millis() as u64);
            node_selector.record_success(peer);
            // Re-evaluate best node
            if let Some(new_best) = node_selector.select_best() {
                info!(%new_best, "Switched to better exit node");
                *current_exit_peer.lock().unwrap() = Some(new_best);
            }
        }
        Err(_) => {
            node_selector.record_failure(peer);
        }
    }
}
```

**Step 3: Update connection established/closed events**

- `ConnectionEstablished` → `node_selector.set_connected(peer, true)`
- `ConnectionClosed` → `node_selector.set_connected(peer, false)`, re-evaluate

**Step 4: Add 1-second traffic sampling interval**

```rust
let mut traffic_interval = time::interval(Duration::from_secs(1));
// In select! loop:
_ = traffic_interval.tick() => {
    let snap = traffic_counter.snapshot();
    // Calculate speed from delta since last snapshot
    let upload_speed = snap.bytes_sent.saturating_sub(last_bytes_sent);
    let download_speed = snap.bytes_received.saturating_sub(last_bytes_received);
    last_bytes_sent = snap.bytes_sent;
    last_bytes_received = snap.bytes_received;
    // Update shared state
    let mut state = shared_state.lock().unwrap();
    state.traffic_stats.upload_speed = upload_speed;
    state.traffic_stats.download_speed = download_speed;
    state.traffic_stats.total_uploaded = snap.bytes_sent;
    state.traffic_stats.total_downloaded = snap.bytes_received;
    state.traffic_stats.active_connections = snap.active_connections;
}
```

**Step 5: Use Arc<Mutex<Option<PeerId>>> for current exit node**

- Proxy tasks read from this shared ref for new connections
- Update proxy start to use shared ref instead of fixed PeerId

**Step 6: Build verification**

**Step 7: Commit**

```bash
git add clash-app/src-tauri/src/swarm_task.rs
git commit -m "feat(swarm): integrate traffic counting and node selection"
```

---

### Task 5: Update proxy to use dynamic exit node selection

**Files:**
- Modify: `clash-lib/src/proxy/socks5.rs`
- Modify: `clash-lib/src/proxy/http_connect.rs`

**Step 1: Change proxy functions to accept `Arc<Mutex<Option<PeerId>>>` instead of fixed PeerId**

Each new connection reads the current best exit node from the shared ref. If no exit node is available, return an error to the client.

**Step 2: Build verification**

**Step 3: Commit**

```bash
git add clash-lib/src/proxy/
git commit -m "feat(proxy): use dynamic exit node selection"
```

---

### Task 6: Extend AppStatus with peer latency info

**Files:**
- Modify: `clash-app/src-tauri/src/state.rs` — extend PeerInfo or peers list
- Modify: `clash-app/src-tauri/src/swarm_task.rs` — populate peer latency in status updates
- Modify: `clash-app/src/types.rs` — add latency_ms and is_selected to PeerInfo
- Modify: `clash-app/src/pages/nodes.rs` — display latency and selected indicator

**Step 1: Add fields to backend PeerInfo struct**

If PeerInfo exists in state.rs, add `latency_ms: Option<u64>` and `is_selected: bool`.
If peers is just a `Vec<String>`, change to `Vec<PeerInfo>` with `{ peer_id, latency_ms, is_selected }`.

**Step 2: Populate from NodeSelector in swarm_task status updates**

**Step 3: Update frontend types.rs to match**

**Step 4: Update nodes.rs to display latency and selected badge**

**Step 5: Build verification (workspace + WASM)**

**Step 6: Commit**

```bash
git add clash-app/
git commit -m "feat(ui): show peer latency and selected exit node"
```

---

### Task 7: Persist last_exit_node and build verification

**Files:**
- Modify: `clash-lib/src/network_id.rs` — add `last_exit_node: Option<String>` to NetworkConfig
- Modify: `clash-app/src-tauri/src/swarm_task.rs` — persist on node change, load on startup

**Step 1: Add field to NetworkConfig**

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub last_exit_node: Option<String>,
```

**Step 2: Update constructors (default/public/private) to include `last_exit_node: None`**

**Step 3: Save when current node changes**

**Step 4: Load on startup and set as preferred in NodeSelector**

**Step 5: Full build verification (workspace + WASM)**

**Step 6: Commit**

```bash
git add clash-lib/src/network_id.rs clash-app/src-tauri/src/swarm_task.rs
git commit -m "feat(config): persist last selected exit node"
```
