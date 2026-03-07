# P7: System Proxy Auto-Configuration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add cross-platform system proxy auto-configuration (macOS/Windows/Linux) with manual toggle in Settings.

**Architecture:** Platform-specific implementations behind a unified API in clash-lib, integrated via Tauri IPC commands, with Drop-based safety guard for cleanup on exit.

**Tech Stack:** networksetup (macOS), winreg + wininet (Windows), gsettings (Linux)

---

### Task 1: Create sys_proxy module with cross-platform API

**Files:**
- Create: `clash-lib/src/sys_proxy.rs`
- Modify: `clash-lib/src/lib.rs` — add `pub mod sys_proxy`
- Modify: `clash-lib/Cargo.toml` — add `winreg` dependency (Windows only)

**Implementation:**

```rust
// clash-lib/src/sys_proxy.rs
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct SystemProxyState {
    pub enabled: bool,
    pub http_proxy: Option<String>,
    pub socks_proxy: Option<String>,
}

pub fn set_system_proxy(http_port: u16, socks_port: u16) -> Result<()> {
    platform::set_proxy(http_port, socks_port)
}

pub fn clear_system_proxy() -> Result<()> {
    platform::clear_proxy()
}

pub fn get_system_proxy() -> Result<SystemProxyState> {
    platform::get_proxy()
}

/// Drop guard that clears system proxy on drop.
pub struct ProxyGuard {
    active: bool,
}

impl ProxyGuard {
    pub fn new() -> Self { Self { active: false } }
    pub fn activate(&mut self) { self.active = true; }
    pub fn deactivate(&mut self) { self.active = false; }
    pub fn is_active(&self) -> bool { self.active }
}

impl Drop for ProxyGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = clear_system_proxy();
        }
    }
}

// --- macOS ---
#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::process::Command;

    fn get_active_interface() -> Result<String> {
        // Try Wi-Fi first, then Ethernet
        let output = Command::new("networksetup")
            .args(["-listallnetworkservices"])
            .output()?;
        let services = String::from_utf8_lossy(&output.stdout);
        for service in ["Wi-Fi", "Ethernet", "USB 10/100/1000 LAN"] {
            if services.contains(service) {
                return Ok(service.to_string());
            }
        }
        anyhow::bail!("No active network interface found")
    }

    pub fn set_proxy(http_port: u16, socks_port: u16) -> Result<()> {
        let iface = get_active_interface()?;
        // HTTP proxy
        Command::new("networksetup")
            .args(["-setwebproxy", &iface, "127.0.0.1", &http_port.to_string()])
            .output()?;
        Command::new("networksetup")
            .args(["-setwebproxystate", &iface, "on"])
            .output()?;
        // HTTPS proxy (same port as HTTP CONNECT)
        Command::new("networksetup")
            .args(["-setsecurewebproxy", &iface, "127.0.0.1", &http_port.to_string()])
            .output()?;
        Command::new("networksetup")
            .args(["-setsecurewebproxystate", &iface, "on"])
            .output()?;
        // SOCKS proxy
        Command::new("networksetup")
            .args(["-setsocksfirewallproxy", &iface, "127.0.0.1", &socks_port.to_string()])
            .output()?;
        Command::new("networksetup")
            .args(["-setsocksfirewallproxystate", &iface, "on"])
            .output()?;
        Ok(())
    }

    pub fn clear_proxy() -> Result<()> {
        let iface = get_active_interface()?;
        Command::new("networksetup").args(["-setwebproxystate", &iface, "off"]).output()?;
        Command::new("networksetup").args(["-setsecurewebproxystate", &iface, "off"]).output()?;
        Command::new("networksetup").args(["-setsocksfirewallproxystate", &iface, "off"]).output()?;
        Ok(())
    }

    pub fn get_proxy() -> Result<SystemProxyState> {
        let iface = get_active_interface()?;
        let output = Command::new("networksetup")
            .args(["-getwebproxy", &iface])
            .output()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let enabled = text.contains("Enabled: Yes");
        Ok(SystemProxyState {
            enabled,
            http_proxy: if enabled { Some("127.0.0.1".to_string()) } else { None },
            socks_proxy: None,
        })
    }
}

// --- Windows ---
#[cfg(target_os = "windows")]
mod platform {
    use super::*;

    pub fn set_proxy(http_port: u16, _socks_port: u16) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")?;
        key.set_value("ProxyEnable", &1u32)?;
        key.set_value("ProxyServer", &format!("127.0.0.1:{http_port}"))?;
        // Notify system of change
        unsafe {
            winapi::um::wininet::InternetSetOptionW(
                std::ptr::null_mut(),
                winapi::um::wininet::INTERNET_OPTION_SETTINGS_CHANGED,
                std::ptr::null_mut(),
                0,
            );
            winapi::um::wininet::InternetSetOptionW(
                std::ptr::null_mut(),
                winapi::um::wininet::INTERNET_OPTION_REFRESH,
                std::ptr::null_mut(),
                0,
            );
        }
        Ok(())
    }

    pub fn clear_proxy() -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")?;
        key.set_value("ProxyEnable", &0u32)?;
        unsafe {
            winapi::um::wininet::InternetSetOptionW(
                std::ptr::null_mut(),
                winapi::um::wininet::INTERNET_OPTION_SETTINGS_CHANGED,
                std::ptr::null_mut(),
                0,
            );
        }
        Ok(())
    }

    pub fn get_proxy() -> Result<SystemProxyState> {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")?;
        let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
        Ok(SystemProxyState {
            enabled: enabled != 0,
            http_proxy: if enabled != 0 { key.get_value("ProxyServer").ok() } else { None },
            socks_proxy: None,
        })
    }
}

// --- Linux ---
#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::process::Command;

    pub fn set_proxy(http_port: u16, socks_port: u16) -> Result<()> {
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy", "mode", "manual"]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.http", "host", "127.0.0.1"]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.http", "port", &http_port.to_string()]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.https", "host", "127.0.0.1"]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.https", "port", &http_port.to_string()]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.socks", "host", "127.0.0.1"]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.socks", "port", &socks_port.to_string()]).output();
        Ok(())
    }

    pub fn clear_proxy() -> Result<()> {
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy", "mode", "none"]).output();
        Ok(())
    }

    pub fn get_proxy() -> Result<SystemProxyState> {
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.system.proxy", "mode"])
            .output();
        let enabled = output.map(|o| String::from_utf8_lossy(&o.stdout).trim() == "'manual'").unwrap_or(false);
        Ok(SystemProxyState {
            enabled,
            http_proxy: None,
            socks_proxy: None,
        })
    }
}
```

**Build verification:** `cargo build --workspace`

**Commit:** `feat(sys-proxy): add cross-platform system proxy configuration`

---

### Task 2: Add backend commands and state

**Files:**
- Modify: `clash-app/src-tauri/src/state.rs` — add `system_proxy_enabled`, new commands
- Modify: `clash-app/src-tauri/src/commands.rs` — add `set_system_proxy`, `clear_system_proxy`
- Modify: `clash-app/src-tauri/src/lib.rs` — register commands, add exit cleanup

**State changes:**
```rust
// state.rs - SharedState
pub system_proxy_enabled: bool,

// state.rs - AppCommand
AppCommand::SetSystemProxy,
AppCommand::ClearSystemProxy,
```

**Commands:**
```rust
#[tauri::command]
pub async fn set_system_proxy(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.cmd_tx.send(AppCommand::SetSystemProxy).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_system_proxy(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.cmd_tx.send(AppCommand::ClearSystemProxy).await.map_err(|e| e.to_string())
}
```

**Exit cleanup in lib.rs:** Register on_exit to clear system proxy.

**Build verification:** `cargo build -p clash-app`

**Commit:** `feat(sys-proxy): add Tauri backend commands for system proxy`

---

### Task 3: Handle commands in swarm_task

**Files:**
- Modify: `clash-app/src-tauri/src/swarm_task.rs`

**Add ProxyGuard and handle SetSystemProxy/ClearSystemProxy:**

```rust
let mut proxy_guard = clash_lib::sys_proxy::ProxyGuard::new();

// In command handler:
AppCommand::SetSystemProxy => {
    if let Some(ref ps) = shared.read().await.proxy_status {
        if ps.running {
            if let Err(e) = clash_lib::sys_proxy::set_system_proxy(ps.http_port, ps.socks5_port) {
                warn!("Failed to set system proxy: {e}");
            } else {
                proxy_guard.activate();
                shared.write().await.system_proxy_enabled = true;
                let _ = app.emit("system-proxy-changed", true);
            }
        }
    }
}
AppCommand::ClearSystemProxy => {
    if let Err(e) = clash_lib::sys_proxy::clear_system_proxy() {
        warn!("Failed to clear system proxy: {e}");
    }
    proxy_guard.deactivate();
    shared.write().await.system_proxy_enabled = false;
    let _ = app.emit("system-proxy-changed", false);
}
```

**Build verification:** `cargo build -p clash-app`

**Commit:** `feat(sys-proxy): handle system proxy commands in swarm_task`

---

### Task 4: Frontend integration

**Files:**
- Modify: `clash-app/src/types.rs` — add `system_proxy_enabled`
- Modify: `clash-app/src/api.rs` — add `set_system_proxy()`, `clear_system_proxy()`
- Modify: `clash-app/src/pages/settings.rs` — add System Proxy toggle
- Modify: `clash-app/src/components/status_bar.rs` — add SYS indicator

**Settings UI:**
- New card below Proxy section: "System Proxy"
- Toggle switch (same style as Proxy toggle)
- Disabled state when proxy not running
- Status text showing current state

**StatusBar:**
- Add SYS: ON/OFF between NET and Traffic indicators

**Build verification:** `cargo build --workspace && trunk build`

**Commit:** `feat(sys-proxy): add system proxy toggle to Settings and StatusBar`
