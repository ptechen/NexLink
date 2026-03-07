# Clash - P2P Network Proxy App Design

## Overview

A hybrid P2P network proxy application supporting both private networking and public node sharing.
Built with libp2p + Tauri v2 + Leptos.

## Requirements

| Dimension | Decision |
|-----------|----------|
| Usage Mode | Hybrid: private networking + public node sharing |
| Target Platform | Desktop first (Win/Mac/Linux), mobile later |
| Node Discovery | Lightweight centralized coordination (Rendezvous/Relay) |
| Proxy Protocols | HTTP + SOCKS5, TUN later |
| Identity | Decentralized identity (PeerId + keypair) |
| Incentives | Not implemented in v1 |
| Encryption | libp2p Noise protocol |
| Tech Stack | libp2p + Tauri v2 + Leptos |

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Tauri v2 App                    │
│  ┌───────────────────────────────────────────┐  │
│  │           Leptos Frontend (WASM)          │  │
│  │  ┌─────────┐ ┌──────────┐ ┌───────────┐  │  │
│  │  │ Nodes   │ │ Traffic  │ │ Settings  │  │  │
│  │  └─────────┘ └──────────┘ └───────────┘  │  │
│  └──────────────────┬────────────────────────┘  │
│                     │ Tauri IPC (Commands)       │
│  ┌──────────────────┴────────────────────────┐  │
│  │           Rust Backend Core               │  │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────┐ │  │
│  │  │ P2P      │ │ Proxy    │ │ Identity  │ │  │
│  │  │ (libp2p) │ │(SOCKS/HTTP)│ │ (keypair) │ │  │
│  │  └─────┬────┘ └─────┬────┘ └───────────┘ │  │
│  │        │             │                    │  │
│  │  ┌─────┴─────────────┴──────────────┐     │  │
│  │  │        Traffic Router             │     │  │
│  │  │  Local Proxy ←→ P2P Stream ←→ Exit│     │  │
│  │  └──────────────────────────────────┘     │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
         │                          │
         │ QUIC/TCP (Noise)         │
         ▼                          ▼
  ┌──────────────┐          ┌──────────────┐
  │  Relay/       │          │  Peer Nodes  │
  │  Rendezvous  │          └──────────────┘
  └──────────────┘
```

## Workspace Structure

```
clash/
├── Cargo.toml              # workspace root
├── clash-lib/              # Core library (P2P + proxy + identity)
│   └── src/
│       ├── identity/       # Keypair generation, storage, PeerId
│       ├── network/        # libp2p Swarm, protocols, behaviours
│       ├── proxy/          # SOCKS5 + HTTP proxy
│       ├── router/         # Traffic routing: local proxy ←→ P2P stream
│       └── config/         # Configuration
├── clash-node/             # Headless exit node binary
├── clash-app/              # Tauri desktop app
│   ├── src/                # Tauri commands
│   └── frontend/           # Leptos WASM frontend
└── clash-relay/            # Rendezvous/Relay signaling server
```

## P2P Network Layer (libp2p)

### Protocol Stack

| Layer | Choice | Notes |
|-------|--------|-------|
| Transport | QUIC (quinn) | Built-in TLS 1.3, 0-RTT, NAT friendly |
| Encryption | Noise XX | libp2p default, mutual auth via PeerId |
| Mux | yamux | For TCP fallback; QUIC has built-in mux |
| Discovery | Rendezvous | Register/query nodes via signaling server |
| NAT | AutoNAT + Relay v2 | Auto-detect NAT, relay fallback |
| App Protocol | `/clash/proxy/1.0.0` | Custom stream protocol for proxy traffic |

### Proxy Data Flow

P2P stream acts as a transparent byte tunnel. No custom framing.

```
User App          Client Node              Exit Node           Target
  │                   │                       │                  │
  │─ SOCKS5 ────────►│                       │                  │
  │◄─ SOCKS5 reply ──│                       │                  │
  │─ CONNECT target ►│                       │                  │
  │                   │─ Open P2P Stream ────►│                  │
  │                   │  (send target addr)   │─ TCP connect ──►│
  │◄══ raw bytes ════►│◄══ P2P Stream ═══════►│◄══ raw bytes ══►│
```

1. Local SOCKS5/HTTP proxy receives connection, parses target address
2. Client opens libp2p stream to exit node
3. Sends target address as a single line at stream start (e.g. `example.com:443\n`)
4. Exit node reads target, establishes TCP connection
5. Bidirectional raw byte forwarding thereafter

### Private vs Public Networks

- **Private**: Nodes share a `NetworkId` (derived from group key), register under that namespace in Rendezvous
- **Public**: Global namespace `clash-public`, any node can register as public exit

## Tauri + Leptos Frontend

### Communication Pattern

- **Commands** (request-response): User actions like start/stop proxy, switch nodes
- **Events** (server push): Backend pushes status changes, traffic stats, peer discovery

### Key Commands

- `start_proxy` / `stop_proxy` - Start/stop local SOCKS5+HTTP listener
- `list_nodes` - Query discovered peers from libp2p
- `connect_node(peer_id)` - Select exit node
- `get_identity` - Get current PeerId and identity info
- `join_network(network_id)` / `leave_network` - Private network management

### Key Events

- `proxy_status` - Proxy state changes
- `traffic_update` - Real-time traffic statistics
- `peer_discovered` / `peer_lost` - Node discovery notifications

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `libp2p` | P2P core (noise, quic, yamux, rendezvous, relay, autonat) |
| `tauri` v2 | Desktop app framework |
| `leptos` | Reactive WASM frontend |
| `tokio` | Async runtime |
| `fast-socks5` | SOCKS5 proxy |
| `hyper` | HTTP CONNECT proxy |
| `serde` / `serde_json` | Serialization |
| `tracing` | Logging |
| `dirs` | Cross-platform config directories |

## Implementation Phases

| Phase | Content | Deliverable |
|-------|---------|-------------|
| P1 | Workspace, clash-lib structure, identity, libp2p Swarm | Two nodes discover and connect |
| P2 | SOCKS5/HTTP local proxy, P2P stream forwarding, exit node TCP relay | Traffic proxied through P2P |
| P3 | clash-relay, Rendezvous registration/discovery, Relay fallback | Cross-network discovery + NAT traversal |
| P4 | Tauri + Leptos integration, IPC commands, basic UI | Usable desktop GUI |
| P5 | NetworkId, namespace isolation, group invitations | Private/public network switching |
| P6 | Traffic stats, multi-exit load balancing, node scoring, config persistence | Production ready |
