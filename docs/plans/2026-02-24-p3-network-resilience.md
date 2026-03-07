# P3: Network Resilience Enhancement Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enhance the P2P network with AutoNAT detection, relay circuit event handling, smart connection fallback, and relay server hardening.

**Architecture:** Add AutoNAT v1 behaviour to client nodes for automatic NAT status detection; add AutoNAT v2 server to relay for dial-back probing. Handle relay::client events to track circuit reservations. Configure relay server with proper rate limits, circuit duration, and connection caps. Handle `OutgoingConnectionError` to retry via relay circuit when direct dial fails.

**Tech Stack:** libp2p 0.56 (autonat, relay v2), tokio

---

### Task 1: Add AutoNAT to ClashBehaviour and RelayBehaviour

**Files:**
- Modify: `clash-lib/src/network/behaviour.rs`
- Modify: `clash-lib/src/network/swarm.rs`

**Context:** `autonat` feature is already enabled in the workspace `libp2p` dependency. AutoNAT v1 (`autonat::Behaviour`) acts as both client and server, providing `NatStatus` (Public/Private/Unknown). The relay server should also include AutoNAT v2 server so it can perform dial-back probes for client nodes.

**Step 1: Add autonat to ClashBehaviour**

Modify `clash-lib/src/network/behaviour.rs`:

```rust
use libp2p::swarm::NetworkBehaviour;
use libp2p::{autonat, identify, ping, relay, rendezvous};
use libp2p_stream as stream;

/// Behaviour for client/exit nodes — connects to relay, discovers peers
#[derive(NetworkBehaviour)]
pub struct ClashBehaviour {
    pub relay_client: relay::client::Behaviour,
    pub identify: identify::Behaviour,
    pub rendezvous_client: rendezvous::client::Behaviour,
    pub ping: ping::Behaviour,
    pub stream: stream::Behaviour,
    pub autonat: autonat::Behaviour,
}

/// Behaviour for the relay/rendezvous server
#[derive(NetworkBehaviour)]
pub struct RelayBehaviour {
    pub relay: relay::Behaviour,
    pub identify: identify::Behaviour,
    pub rendezvous_server: rendezvous::server::Behaviour,
    pub ping: ping::Behaviour,
    pub autonat: autonat::Behaviour,
}
```

**Step 2: Initialize autonat in swarm builders**

Modify `clash-lib/src/network/swarm.rs`:

Add `use libp2p::autonat;` and `use std::time::Duration;` to imports.

In `build_client_swarm`, add to the ClashBehaviour init inside `with_behaviour(|key, relay_behaviour|`:

```rust
autonat: autonat::Behaviour::new(
    key.public().to_peer_id(),
    autonat::Config {
        boot_delay: Duration::from_secs(10),
        retry_interval: Duration::from_secs(60),
        only_global_ips: false, // allow local IPs for dev/testing
        ..Default::default()
    },
),
```

In `build_relay_swarm`, add to the RelayBehaviour init inside `with_behaviour(|key|`:

```rust
autonat: autonat::Behaviour::new(
    key.public().to_peer_id(),
    autonat::Config {
        only_global_ips: false,
        ..Default::default()
    },
),
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 4: Commit**

```bash
git add clash-lib/src/network/behaviour.rs clash-lib/src/network/swarm.rs
git commit -m "feat(network): add AutoNAT behaviour for NAT status detection"
```

---

### Task 2: Handle AutoNAT and Relay Client Events in clash-node

**Files:**
- Modify: `clash-node/src/main.rs`

**Context:** The `#[derive(NetworkBehaviour)]` macro generates event enum variants from field names using PascalCase. With the new `autonat` field, `ClashBehaviourEvent::Autonat(autonat::Event)` is now available. Similarly, `RelayClient(relay::client::Event)` has always existed but was caught by the `_ => {}` wildcard.

**Step 1: Add autonat and relay client imports**

Add to the imports at the top of `clash-node/src/main.rs`:

```rust
use libp2p::{autonat, relay};
```

**Step 2: Add AutoNAT event handler**

In the main event loop, add before the `_ => {}` catch-all:

```rust
SwarmEvent::Behaviour(ClashBehaviourEvent::Autonat(event)) => {
    match event {
        autonat::Event::StatusChanged { old, new } => {
            info!(?old, ?new, "NAT status changed");
            if let autonat::NatStatus::Public(addr) = &new {
                info!(%addr, "Publicly reachable");
            }
        }
        _ => {
            tracing::debug!("AutoNAT: {event:?}");
        }
    }
}
```

**Step 3: Add relay client event handler**

Add before the `_ => {}` catch-all:

```rust
SwarmEvent::Behaviour(ClashBehaviourEvent::RelayClient(event)) => {
    match event {
        relay::client::Event::ReservationReqAccepted {
            relay_peer_id: peer,
            renewal,
            ..
        } => {
            info!(%peer, %renewal, "Relay reservation accepted");
        }
        relay::client::Event::InboundCircuitEstablished {
            src_peer_id,
            ..
        } => {
            info!(%src_peer_id, "Inbound circuit through relay");
        }
        relay::client::Event::OutboundCircuitEstablished {
            relay_peer_id: peer,
            ..
        } => {
            info!(%peer, "Outbound circuit through relay");
        }
    }
}
```

**Step 4: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 5: Commit**

```bash
git add clash-node/src/main.rs
git commit -m "feat(node): handle AutoNAT status and relay circuit events"
```

---

### Task 3: Smart Peer Dialing with Relay Fallback

**Files:**
- Modify: `clash-node/src/main.rs`

**Context:** Currently, when a peer is discovered via rendezvous, we `swarm.dial(peer)` which relies on behaviours to supply addresses. If the peer registered with a relay circuit address, libp2p tries both direct and circuit addresses concurrently. However, if the initial dial completely fails (e.g., all addresses exhausted), we have no retry logic. We need to handle `OutgoingConnectionError` and retry via explicit relay circuit address.

**Step 1: Add outgoing connection error handling with relay fallback**

In the main event loop, add a new arm for `OutgoingConnectionError`:

```rust
SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
    if let Some(peer) = peer_id {
        warn!(%peer, "Outgoing connection failed: {error}");
        // Don't retry relay connections
        if peer != relay_peer_id {
            // Retry via relay circuit
            let circuit_addr: Multiaddr = format!(
                "{relay_addr}/p2p-circuit/p2p/{peer}"
            ).parse().expect("valid circuit addr");
            info!(%peer, "Retrying via relay circuit");
            if let Err(e) = swarm.dial(
                libp2p::swarm::dial_opts::DialOpts::peer_id(peer)
                    .addresses(vec![circuit_addr])
                    .condition(libp2p::swarm::dial_opts::PeerCondition::Always)
                    .build()
            ) {
                warn!(%peer, "Relay circuit dial failed: {e}");
            }
        }
    }
}
```

**Step 2: Also handle ListenerError for relay circuit listener issues**

Add a new arm:

```rust
SwarmEvent::ListenerError { listener_id, error } => {
    warn!(?listener_id, "Listener error: {error}");
}
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 4: Commit**

```bash
git add clash-node/src/main.rs
git commit -m "feat(node): add relay circuit fallback on connection failure"
```

---

### Task 4: Relay Server Configuration and Hardening

**Files:**
- Modify: `clash-relay/src/main.rs`

**Context:** The relay server currently uses `Default::default()` for both `relay::Config` and `rendezvous::server::Config`. We should configure explicit limits for production readiness: reservation duration, max circuits, rate limiting. Also add CLI flags for the listen address (already exists) and data directory, plus logging for AutoNAT events on the relay.

**Step 1: Add relay configuration with explicit limits**

Replace the `relay::Behaviour::new(key.public().to_peer_id(), Default::default())` call in `build_relay_swarm`. But since the swarm builder is in `clash-lib`, we need to make the relay config configurable.

Modify `clash-lib/src/network/swarm.rs` to accept an optional `relay::Config`:

Change the function signature:

```rust
pub async fn build_relay_swarm(
    identity: &NodeIdentity,
    relay_config: relay::Config,
) -> Result<Swarm<RelayBehaviour>> {
```

And use it:

```rust
relay: relay::Behaviour::new(key.public().to_peer_id(), relay_config),
```

**Step 2: Update clash-relay/src/main.rs with configuration**

Add CLI flags and configure limits:

```rust
#[derive(Parser)]
#[command(name = "clash-relay", about = "Clash P2P relay/rendezvous server")]
struct Cli {
    /// Listen address
    #[arg(short, long, default_value = "/ip4/0.0.0.0/udp/4001/quic-v1")]
    listen: String,

    /// Max concurrent relay reservations
    #[arg(long, default_value_t = 128)]
    max_reservations: usize,

    /// Max circuits per peer
    #[arg(long, default_value_t = 4)]
    max_circuits_per_peer: usize,

    /// Max circuit duration in seconds
    #[arg(long, default_value_t = 300)]
    max_circuit_duration_secs: u64,
}
```

Build the relay config:

```rust
use std::time::Duration;

let relay_config = libp2p::relay::Config {
    max_reservations: cli.max_reservations,
    max_circuits_per_peer: cli.max_circuits_per_peer,
    max_circuit_duration: Duration::from_secs(cli.max_circuit_duration_secs),
    ..Default::default()
};

let mut swarm = build_relay_swarm(&identity, relay_config).await?;
```

**Step 3: Add AutoNAT event logging to relay**

Add event handler for AutoNAT in the relay's match block:

```rust
RelayBehaviourEvent::Autonat(e) => {
    info!("AutoNAT: {e:?}");
}
```

**Step 4: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 5: Commit**

```bash
git add clash-lib/src/network/swarm.rs clash-relay/src/main.rs
git commit -m "feat(relay): add configurable relay limits and AutoNAT server"
```

---

### Task 5: Integration Test — AutoNAT + Relay Circuit Proxy

**Files:**
- No new files (manual test)

**Context:** Verify the full enhanced network stack: relay starts with configured limits, nodes detect NAT status via AutoNAT, nodes establish relay reservations, and proxy traffic works through relay circuits.

**Step 1: Build the project**

```bash
cargo build 2>&1 | tail -3
```

**Step 2: Start relay**

```bash
rm -f /tmp/relay.log /tmp/exit.log /tmp/client.log
rm -rf ~/.clash/exit_a ~/.clash/client_a

target/debug/clash-relay --max-circuit-duration-secs 600 > /tmp/relay.log 2>&1 &
sleep 3

RELAY_PEER_ID=$(perl -pe 's/\e\[\d+(;\d+)*m//g' /tmp/relay.log | grep -oE '12D3KooW[A-Za-z0-9]+' | head -1)
RELAY_ADDR="/ip4/127.0.0.1/udp/4001/quic-v1/p2p/${RELAY_PEER_ID}"
echo "Relay: $RELAY_PEER_ID"
```

**Step 3: Start exit node**

```bash
target/debug/clash-node --relay "$RELAY_ADDR" --exit-node --data-dir ~/.clash/exit_a > /tmp/exit.log 2>&1 &
sleep 5
```

**Step 4: Start client node**

```bash
target/debug/clash-node --relay "$RELAY_ADDR" --data-dir ~/.clash/client_a --socks5-port 11080 --http-port 18080 > /tmp/client.log 2>&1 &
sleep 10
```

**Step 5: Verify relay reservation**

```bash
# Check relay reservation is accepted in exit node log
perl -pe 's/\e\[\d+(;\d+)*m//g' /tmp/exit.log | grep -i "reservation"
# Expected: "Relay reservation accepted"
```

**Step 6: Test SOCKS5 proxy**

```bash
curl --max-time 15 --socks5 127.0.0.1:11080 http://httpbin.org/ip
# Expected: {"origin": "..."}
```

**Step 7: Test HTTP CONNECT proxy**

```bash
curl --max-time 15 --proxy http://127.0.0.1:18080 https://httpbin.org/ip
# Expected: {"origin": "..."}
```

**Step 8: Check NAT status in logs**

```bash
perl -pe 's/\e\[\d+(;\d+)*m//g' /tmp/client.log | grep -i "nat"
# Expected: "NAT status changed" log entries
```

**Step 9: Clean up and commit**

```bash
pkill -f "clash-relay"; pkill -f "clash-node"
git add -A
git commit -m "feat: complete P3 network resilience enhancements"
```
