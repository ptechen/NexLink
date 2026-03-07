# P5: NetworkId & Namespace Isolation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add private network support via shared-password-derived NetworkId, allowing nodes to form isolated groups using distinct Rendezvous namespaces.

**Architecture:** Users input network name + password, HKDF-SHA256 derives a deterministic NetworkId used as Rendezvous namespace. Config persisted to `network.json`. Public network = default `clash-public` namespace.

**Tech Stack:** `hkdf` + `sha2` crates for key derivation, existing Tauri IPC + Leptos frontend

---

### Task 1: Add `hkdf` and `sha2` to clash-lib, create `network_id` module

**Files:**
- Modify: `Cargo.toml` (workspace root, line 31-32 area) — add workspace deps
- Modify: `clash-lib/Cargo.toml` (line 16 area) — add deps
- Create: `clash-lib/src/network_id.rs`
- Modify: `clash-lib/src/lib.rs` — export module

**Step 1: Add dependencies**

In workspace `Cargo.toml`, add to `[workspace.dependencies]`:
```toml
hkdf = "0.12"
sha2 = "0.10"
hex = "0.4"
```

In `clash-lib/Cargo.toml`, add:
```toml
hkdf = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
```

**Step 2: Create `clash-lib/src/network_id.rs`**

```rust
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::Path;

const SALT: &[u8] = b"clash-network-v1";
const INFO: &[u8] = b"rendezvous-namespace";

/// Derive a deterministic NetworkId from name + password via HKDF-SHA256.
pub fn derive_network_id(name: &str, password: &str) -> String {
    let ikm = format!("{name}:{password}");
    let hk = Hkdf::<Sha256>::new(Some(SALT), ikm.as_bytes());
    let mut okm = [0u8; 16];
    hk.expand(INFO, &mut okm).expect("16 bytes is valid for HKDF");
    hex::encode(okm)
}

/// Persistent network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// "public" or "private"
    pub mode: String,
    /// Human-readable network name (only for private)
    pub network_name: Option<String>,
    /// Derived network ID hex (only for private)
    pub network_id: Option<String>,
    /// Rendezvous namespace
    pub namespace: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: "public".to_string(),
            network_name: None,
            network_id: None,
            namespace: "clash-public".to_string(),
        }
    }
}

impl NetworkConfig {
    pub fn public() -> Self {
        Self::default()
    }

    pub fn private(name: &str, password: &str) -> Self {
        let network_id = derive_network_id(name, password);
        let namespace = format!("clash-{network_id}");
        Self {
            mode: "private".to_string(),
            network_name: Some(name.to_string()),
            network_id: Some(network_id),
            namespace,
        }
    }

    pub fn is_private(&self) -> bool {
        self.mode == "private"
    }
}

const CONFIG_FILE: &str = "network.json";

pub fn load_network_config(data_dir: &Path) -> NetworkConfig {
    let path = data_dir.join(CONFIG_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => NetworkConfig::default(),
    }
}

pub fn save_network_config(data_dir: &Path, config: &NetworkConfig) -> anyhow::Result<()> {
    let path = data_dir.join(CONFIG_FILE);
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn remove_network_config(data_dir: &Path) {
    let path = data_dir.join(CONFIG_FILE);
    let _ = std::fs::remove_file(path);
}
```

**Step 3: Export module in `clash-lib/src/lib.rs`**

Add `pub mod network_id;` line.

**Step 4: Verify build**

Run: `cargo build -p clash-lib`

**Step 5: Commit**

```bash
git add clash-lib/src/network_id.rs clash-lib/src/lib.rs clash-lib/Cargo.toml Cargo.toml
git commit -m "feat(lib): add network_id module with HKDF derivation and config persistence"
```

---

### Task 2: Add backend state and commands for network switching

**Files:**
- Modify: `clash-app/src-tauri/src/state.rs`
- Modify: `clash-app/src-tauri/src/commands.rs`
- Modify: `clash-app/src-tauri/src/lib.rs` — register new commands

**Step 1: Update `state.rs`**

Add to `SharedState`:
```rust
pub network_mode: String,       // "public" or "private"
pub network_name: Option<String>, // human-readable name for private
```

Add to `AppCommand` enum:
```rust
JoinNetwork { name: String, password: String },
LeaveNetwork,
```

**Step 2: Add commands to `commands.rs`**

```rust
#[tauri::command]
pub async fn join_network(
    state: State<'_, AppState>,
    name: String,
    password: String,
) -> Result<(), String> {
    state
        .cmd_tx
        .send(AppCommand::JoinNetwork { name, password })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn leave_network(state: State<'_, AppState>) -> Result<(), String> {
    state
        .cmd_tx
        .send(AppCommand::LeaveNetwork)
        .await
        .map_err(|e| e.to_string())
}
```

**Step 3: Register in `lib.rs`**

Add `commands::join_network` and `commands::leave_network` to `invoke_handler`.

Also update initial `SharedState` to include `network_mode: "public".to_string()` and `network_name: None`.

**Step 4: Verify build**

Run: `cargo build -p clash-app`

**Step 5: Commit**

```bash
git add clash-app/src-tauri/src/state.rs clash-app/src-tauri/src/commands.rs clash-app/src-tauri/src/lib.rs
git commit -m "feat(app): add join_network/leave_network commands and state fields"
```

---

### Task 3: Implement network switching in swarm_task

**Files:**
- Modify: `clash-app/src-tauri/src/swarm_task.rs`

**Step 1: Load network config on startup**

After identity load, before swarm event loop:
```rust
let data_path = std::path::Path::new(&data_dir_str);
let net_config = clash_lib::network_id::load_network_config(data_path);
let initial_namespace = net_config.namespace.clone();

// Update shared state
{
    let mut state = shared.write().await;
    state.namespace = initial_namespace;
    state.network_mode = net_config.mode.clone();
    state.network_name = net_config.network_name.clone();
}
```

**Step 2: Handle `JoinNetwork` command**

In the `cmd_rx` match arm, add:
```rust
AppCommand::JoinNetwork { name, password } => {
    let config = clash_lib::network_id::NetworkConfig::private(&name, &password);
    let new_namespace = config.namespace.clone();

    // Stop proxy and disconnect if active
    for h in proxy_handles.drain(..) {
        h.abort();
    }
    if let Some(exit_peer) = connected_exit_peer.take() {
        let _ = swarm.disconnect_peer_id(exit_peer);
    }

    // Save config
    let data_path = std::path::Path::new(&data_dir_str);
    if let Err(e) = clash_lib::network_id::save_network_config(data_path, &config) {
        warn!("Failed to save network config: {e}");
    }

    // Update shared state
    {
        let mut state = shared.write().await;
        state.namespace = new_namespace.clone();
        state.network_mode = "private".to_string();
        state.network_name = Some(name.clone());
        state.connected_peer = None;
        state.discovered_peers.clear();
        state.proxy_status = Some(ProxyStatus { running: false, socks5_port: 1080, http_port: 8080 });
    }

    // Re-register with new namespace
    registered = false;
    if let Some(relay) = relay_peer_id {
        if let Ok(ns) = rendezvous::Namespace::new(new_namespace) {
            match swarm.behaviour_mut().rendezvous_client.register(ns, relay, None) {
                Ok(()) => info!(%name, "Registering with private network"),
                Err(e) => warn!("Failed to register: {e}"),
            }
        }
    }

    let _ = app.emit("network-changed", "private");
    info!(%name, "Joined private network");
}
```

**Step 3: Handle `LeaveNetwork` command**

```rust
AppCommand::LeaveNetwork => {
    let config = clash_lib::network_id::NetworkConfig::public();
    let new_namespace = config.namespace.clone();

    // Stop proxy and disconnect
    for h in proxy_handles.drain(..) {
        h.abort();
    }
    if let Some(exit_peer) = connected_exit_peer.take() {
        let _ = swarm.disconnect_peer_id(exit_peer);
    }

    // Remove config file
    let data_path = std::path::Path::new(&data_dir_str);
    clash_lib::network_id::remove_network_config(data_path);

    // Update shared state
    {
        let mut state = shared.write().await;
        state.namespace = new_namespace.clone();
        state.network_mode = "public".to_string();
        state.network_name = None;
        state.connected_peer = None;
        state.discovered_peers.clear();
        state.proxy_status = Some(ProxyStatus { running: false, socks5_port: 1080, http_port: 8080 });
    }

    // Re-register with public namespace
    registered = false;
    if let Some(relay) = relay_peer_id {
        if let Ok(ns) = rendezvous::Namespace::new(new_namespace) {
            match swarm.behaviour_mut().rendezvous_client.register(ns, relay, None) {
                Ok(()) => info!("Registering with public network"),
                Err(e) => warn!("Failed to register: {e}"),
            }
        }
    }

    let _ = app.emit("network-changed", "public");
    info!("Left private network, back to public");
}
```

**Step 4: Also update discover_tick to use current namespace from shared state**

The discover_tick already reads `shared.read().await.namespace.clone()` so it will automatically use the updated namespace.

**Step 5: Verify build**

Run: `cargo build -p clash-app`

**Step 6: Commit**

```bash
git add clash-app/src-tauri/src/swarm_task.rs
git commit -m "feat(app): implement network switching in swarm task with persistence"
```

---

### Task 4: Update frontend types and API

**Files:**
- Modify: `clash-app/src/types.rs`
- Modify: `clash-app/src/api.rs`

**Step 1: Update `types.rs`**

Add to `AppStatus`:
```rust
pub network_mode: String,
pub network_name: Option<String>,
```

**Step 2: Add API functions in `api.rs`**

```rust
pub async fn join_network(name: &str, password: &str) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args<'a> {
        name: &'a str,
        password: &'a str,
    }
    let args = serde_wasm_bindgen::to_value(&Args { name, password }).map_err(|e| e.to_string())?;
    call_void("join_network", args).await
}

pub async fn leave_network() -> Result<(), String> {
    call_void("leave_network", no_args()).await
}
```

**Step 3: Verify WASM build**

Run: `cd clash-app && trunk build`

**Step 4: Commit**

```bash
git add clash-app/src/types.rs clash-app/src/api.rs
git commit -m "feat(frontend): add network switching types and API calls"
```

---

### Task 5: Update Settings page UI with network management section

**Files:**
- Modify: `clash-app/src/pages/settings.rs`

**Step 1: Add network management signals and UI**

Replace the existing Network section in settings.rs with an expanded section that shows:

1. **Current network status** — badge showing "Public Network" or "Private: {name}"
2. **Join private network form** — name input + password input + "Join" button
3. **Leave network button** — only visible when in private network

New signals needed:
```rust
let (net_name_input, set_net_name) = signal(String::new());
let (net_password_input, set_net_password) = signal(String::new());
let (net_msg, set_net_msg) = signal(Option::<String>::None);
```

Join handler:
```rust
let join_network = move |_| {
    let name = net_name_input.get();
    let password = net_password_input.get();
    spawn_local(async move {
        match api::join_network(&name, &password).await {
            Ok(_) => {
                set_net_msg.set(Some("Joined network".to_string()));
                // Refresh status
                if let Ok(s) = api::get_status().await {
                    set_status.set(Some(s));
                }
            }
            Err(e) => set_net_msg.set(Some(format!("Error: {e}"))),
        }
    });
};
```

Leave handler:
```rust
let leave_network = move |_| {
    spawn_local(async move {
        match api::leave_network().await {
            Ok(_) => {
                set_net_msg.set(Some("Back to public network".to_string()));
                if let Ok(s) = api::get_status().await {
                    set_status.set(Some(s));
                }
            }
            Err(e) => set_net_msg.set(Some(format!("Error: {e}"))),
        }
    });
};
```

UI structure for the Network section:
```html
<!-- Current Network Status -->
<div class="flex items-center gap-2 mb-4">
    <div class="px-2 py-1 rounded text-xs font-medium"
         class:bg-cyber-green/20 class:text-cyber-green={is_public}
         class:bg-cyber-cyan/20 class:text-cyber-cyan={is_private}>
        {network_label}
    </div>
    <!-- Leave button when private -->
    <button on:click=leave_network class="..." show_when_private>
        "Leave Network"
    </button>
</div>

<!-- Join Private Network Form -->
<div class="space-y-3 border-t border-border pt-4">
    <h3>"Join Private Network"</h3>
    <input placeholder="Network name" bind=net_name_input />
    <input type="password" placeholder="Shared password" bind=net_password_input />
    <button on:click=join_network>"Join"</button>
    {net_msg}
</div>

<!-- Relay Server (keep existing) -->
<div class="border-t border-border pt-4">
    <input relay_addr ... />
</div>
```

**Step 2: Verify WASM build**

Run: `cd clash-app && trunk build`

**Step 3: Commit**

```bash
git add clash-app/src/pages/settings.rs
git commit -m "feat(frontend): add network management UI to settings page"
```

---

### Task 6: Update status bar to show network mode

**Files:**
- Modify: `clash-app/src/components/status_bar.rs`

**Step 1: Add network indicator between NAT and Traffic**

After the NAT status section, add:
```rust
<div class="w-px h-3 bg-border"></div>
// Network mode
<div class="flex items-center gap-1.5">
    <span class="text-slate-500">"NET:"</span>
    <span class=move || {
        let mode = status.get().map(|s| s.network_mode).unwrap_or_default();
        if mode == "private" { "text-cyber-cyan" } else { "text-slate-500" }
    }>
        {move || {
            let s = status.get();
            let mode = s.as_ref().map(|s| s.network_mode.as_str()).unwrap_or("public");
            if mode == "private" {
                s.and_then(|s| s.network_name).unwrap_or_else(|| "Private".to_string())
            } else {
                "Public".to_string()
            }
        }}
    </span>
</div>
```

**Step 2: Verify WASM build**

Run: `cd clash-app && trunk build`

**Step 3: Commit**

```bash
git add clash-app/src/components/status_bar.rs
git commit -m "feat(frontend): show network mode in status bar"
```

---

### Task 7: Build verification and integration test

**Step 1: Full workspace build**

Run: `cargo build --workspace`

**Step 2: WASM build**

Run: `cd clash-app && trunk build`

**Step 3: Manual integration test**

1. Start relay: `cargo run -p clash-relay`
2. Start exit: `cargo run -p clash-node -- --exit --relay-addr <addr>`
3. Start GUI: `cd clash-app/src-tauri && cargo tauri dev`
4. Verify: Settings page shows "Public Network" status
5. Join private network with name "test" + password "123"
6. Verify: namespace changes, peer list clears, status bar shows "test"
7. Leave network
8. Verify: back to public, peer list clears

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat(p5): complete network isolation with private/public switching"
```
