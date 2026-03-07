use crate::api;
use crate::types::AppStatus;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn StatusBar() -> impl IntoView {
    let (status, set_status) = signal(Option::<AppStatus>::None);

    // Poll status every 2 seconds
    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                if let Ok(s) = api::get_status().await {
                    set_status.set(Some(s));
                }
                gloo_timers::future::TimeoutFuture::new(2000).await;
            }
        });
    });

    view! {
        <div class="h-8 bg-surface border-t border-border flex items-center px-4 gap-4 text-xs shrink-0">
            // Proxy status
            <div class="flex items-center gap-1.5">
                <div class=move || {
                    let running = status
                        .get()
                        .and_then(|s| s.proxy_status.map(|p| p.running))
                        .unwrap_or(false);
                    if running {
                        "w-1.5 h-1.5 rounded-full bg-cyber-green"
                    } else {
                        "w-1.5 h-1.5 rounded-full bg-slate-600"
                    }
                }></div>
                <span class="text-slate-400">
                    {move || {
                        status
                            .get()
                            .and_then(|s| {
                                s.proxy_status
                                    .map(|p| {
                                        if p.running {
                                            format!(
                                                "UNIFIED:{}",
                                                p.unified_port,
                                            )
                                        } else {
                                            "Proxy off".to_string()
                                        }
                                    })
                            })
                            .unwrap_or_else(|| "Proxy off".to_string())
                    }}
                </span>
            </div>

            <div class="w-px h-3 bg-border"></div>

            // NAT status
            <div class="flex items-center gap-1.5">
                <span class="text-slate-500">"NAT:"</span>
                <span class=move || {
                    let nat = status.get().map(|s| s.nat_status).unwrap_or_default();
                    match nat.as_str() {
                        "Public" => "text-cyber-green",
                        "Private" => "text-cyber-amber",
                        _ => "text-slate-500",
                    }
                }>
                    {move || {
                        status.get().map(|s| s.nat_status).unwrap_or_else(|| "?".to_string())
                    }}
                </span>
            </div>

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

            <div class="w-px h-3 bg-border"></div>

            // System proxy
            <div class="flex items-center gap-1.5">
                <span class="text-slate-500">"SYS:"</span>
                <span class=move || {
                    if status.get().map(|s| s.system_proxy_enabled).unwrap_or(false) {
                        "text-cyber-green"
                    } else {
                        "text-slate-600"
                    }
                }>
                    {move || {
                        if status.get().map(|s| s.system_proxy_enabled).unwrap_or(false) {
                            "ON"
                        } else {
                            "OFF"
                        }
                    }}
                </span>
            </div>

            <div class="w-px h-3 bg-border"></div>

            // Traffic speed
            <div class="flex items-center gap-2">
                <span class="text-cyber-cyan">
                    "\u{2191} "
                    {move || {
                        format_speed(
                            status.get().map(|s| s.traffic.upload_speed).unwrap_or(0),
                        )
                    }}
                </span>
                <span class="text-cyber-green">
                    "\u{2193} "
                    {move || {
                        format_speed(
                            status.get().map(|s| s.traffic.download_speed).unwrap_or(0),
                        )
                    }}
                </span>
            </div>

            <div class="flex-1"></div>

            // Peer ID
            <span class="text-slate-600 font-mono">
                {move || {
                    status
                        .get()
                        .map(|s| {
                            if s.peer_id.len() > 12 {
                                format!("{}...", &s.peer_id[..12])
                            } else {
                                s.peer_id
                            }
                        })
                        .unwrap_or_default()
                }}
            </span>
        </div>
    }
}

fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        return "0 B/s".to_string();
    }
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut size = bytes_per_sec as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} B/s", bytes_per_sec)
    } else {
        format!("{:.1} {}", size, units[unit_idx])
    }
}
