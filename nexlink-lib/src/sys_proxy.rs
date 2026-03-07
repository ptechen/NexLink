use anyhow::Result;

#[derive(Debug, Clone)]
pub struct SystemProxyState {
    pub enabled: bool,
}

/// Set system-wide HTTP and SOCKS5 proxy to localhost with given port.
/// For the unified proxy, both HTTP and SOCKS traffic goes through the same port.
pub fn set_system_proxy(http_port: u16, _socks_port: u16) -> Result<()> {
    platform::set_proxy(http_port)  // Use unified port for both
}

/// Clear (disable) system-wide proxy settings.
pub fn clear_system_proxy() -> Result<()> {
    platform::clear_proxy()
}

/// Query current system proxy state.
pub fn get_system_proxy() -> Result<SystemProxyState> {
    platform::get_proxy()
}

/// RAII guard that clears system proxy on drop.
pub struct ProxyGuard {
    active: bool,
}

impl ProxyGuard {
    pub fn new() -> Self {
        Self { active: false }
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for ProxyGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = clear_system_proxy();
        }
    }
}

// ─── macOS ───────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::process::Command;

    fn get_active_interface() -> Result<String> {
        let output = Command::new("networksetup")
            .args(["-listallnetworkservices"])
            .output()?;
        let services = String::from_utf8_lossy(&output.stdout);
        for service in ["Wi-Fi", "Ethernet", "USB 10/100/1000 LAN"] {
            if services.lines().any(|l| l.trim() == service) {
                return Ok(service.to_string());
            }
        }
        anyhow::bail!("No active network interface found")
    }

    pub fn set_proxy(port: u16) -> Result<()> {
        let iface = get_active_interface()?;
        let port_str = port.to_string();

        // HTTP proxy
        Command::new("networksetup")
            .args(["-setwebproxy", &iface, "127.0.0.1", &port_str])
            .output()?;
        Command::new("networksetup")
            .args(["-setwebproxystate", &iface, "on"])
            .output()?;

        // HTTPS proxy (same port)
        Command::new("networksetup")
            .args(["-setsecurewebproxy", &iface, "127.0.0.1", &port_str])
            .output()?;
        Command::new("networksetup")
            .args(["-setsecurewebproxystate", &iface, "on"])
            .output()?;

        // SOCKS proxy (same unified port)
        Command::new("networksetup")
            .args(["-setsocksfirewallproxy", &iface, "127.0.0.1", &port_str])
            .output()?;
        Command::new("networksetup")
            .args(["-setsocksfirewallproxystate", &iface, "on"])
            .output()?;

        tracing::info!(%iface, %port, "System proxy enabled using unified port");
        Ok(())
    }

    pub fn clear_proxy() -> Result<()> {
        let iface = get_active_interface()?;
        Command::new("networksetup")
            .args(["-setwebproxystate", &iface, "off"])
            .output()?;
        Command::new("networksetup")
            .args(["-setsecurewebproxystate", &iface, "off"])
            .output()?;
        Command::new("networksetup")
            .args(["-setsocksfirewallproxystate", &iface, "off"])
            .output()?;
        tracing::info!(%iface, "System proxy disabled");
        Ok(())
    }

    pub fn get_proxy() -> Result<SystemProxyState> {
        let iface = get_active_interface()?;
        let output = Command::new("networksetup")
            .args(["-getwebproxy", &iface])
            .output()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let enabled = text.contains("Enabled: Yes");
        Ok(SystemProxyState { enabled })
    }
}

// ─── Windows ─────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use super::*;

    pub fn set_proxy(port: u16) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
        )?;
        key.set_value("ProxyEnable", &1u32)?;
        key.set_value("ProxyServer", &format!("127.0.0.1:{port}"))?;

        // Notify system of change
        notify_system_proxy_change();

        tracing::info!(%port, "System proxy enabled using unified port (Windows)");
        Ok(())
    }

    pub fn clear_proxy() -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
        )?;
        key.set_value("ProxyEnable", &0u32)?;

        notify_system_proxy_change();

        tracing::info!("System proxy disabled (Windows)");
        Ok(())
    }

    pub fn get_proxy() -> Result<SystemProxyState> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
        )?;
        let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
        Ok(SystemProxyState {
            enabled: enabled != 0,
        })
    }

    fn notify_system_proxy_change() {
        // INTERNET_OPTION_SETTINGS_CHANGED = 39
        // INTERNET_OPTION_REFRESH = 37
        unsafe {
            winapi::um::wininet::InternetSetOptionW(
                std::ptr::null_mut(),
                39,
                std::ptr::null_mut(),
                0,
            );
            winapi::um::wininet::InternetSetOptionW(
                std::ptr::null_mut(),
                37,
                std::ptr::null_mut(),
                0,
            );
        }
    }
}

// ─── Linux ───────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::process::Command;

    pub fn set_proxy(port: u16) -> Result<()> {
        let port_str = port.to_string();

        // GNOME gsettings - Use the same port for HTTP, HTTPS and SOCKS since it's unified
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "manual"])
            .output();
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.http", "host", "127.0.0.1"])
            .output();
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.http", "port", &port_str])
            .output();
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.https", "host", "127.0.0.1"])
            .output();
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.https", "port", &port_str])
            .output();
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.socks", "host", "127.0.0.1"])
            .output();
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.socks", "port", &port_str])
            .output();

        tracing::info!(%port, "System proxy enabled using unified port (Linux/GNOME)");
        Ok(())
    }

    pub fn clear_proxy() -> Result<()> {
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "none"])
            .output();
        tracing::info!("System proxy disabled (Linux/GNOME)");
        Ok(())
    }

    pub fn get_proxy() -> Result<SystemProxyState> {
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.system.proxy", "mode"])
            .output();
        let enabled = output
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.trim().trim_matches('\'') == "manual"
            })
            .unwrap_or(false);
        Ok(SystemProxyState { enabled })
    }
}
