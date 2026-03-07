use crate::api;
use crate::types::AppStatus;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let (status, set_status) = signal(Option::<AppStatus>::None);
    let (relay_input, set_relay) = signal(String::new());
    let (save_msg, set_save_msg) = signal(Option::<String>::None);
    let (copied, set_copied) = signal(false);

    // Network join form
    let (net_name_input, set_net_name) = signal(String::new());
    let (net_password_input, set_net_password) = signal(String::new());
    let (net_msg, set_net_msg) = signal(Option::<String>::None);

    // Load current settings
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(s) = api::get_status().await {
                set_relay.set(s.relay_addr.clone());
                set_status.set(Some(s));
            }
        });
    });

    let save_settings = move |_| {
        let relay = relay_input.get();
        spawn_local(async move {
            match api::update_config(Some(&relay), None).await {
                Ok(_) => set_save_msg.set(Some("Settings saved".to_string())),
                Err(e) => set_save_msg.set(Some(format!("Error: {e}"))),
            }
        });
    };

    let join_network = move |_| {
        let name = net_name_input.get();
        let password = net_password_input.get();
        if name.is_empty() {
            set_net_msg.set(Some("Error: Network name required".to_string()));
            return;
        }
        spawn_local(async move {
            match api::join_network(&name, &password).await {
                Ok(_) => {
                    set_net_msg.set(Some("Joined network".to_string()));
                    set_net_name.set(String::new());
                    set_net_password.set(String::new());
                    if let Ok(s) = api::get_status().await {
                        set_status.set(Some(s));
                    }
                }
                Err(e) => set_net_msg.set(Some(format!("Error: {e}"))),
            }
        });
    };

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

    let copy_peer_id = move |_| {
        if let Some(s) = status.get() {
            let peer_id = s.peer_id.clone();
            spawn_local(async move {
                let window = web_sys::window().unwrap();
                let navigator = window.navigator();
                let clipboard = navigator.clipboard();
                let _ =
                    wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&peer_id)).await;
                set_copied.set(true);
                gloo_timers::future::TimeoutFuture::new(2000).await;
                set_copied.set(false);
            });
        }
    };

    view! {
        <div class="space-y-6">
            <div>
                <h1 class="text-2xl font-bold text-slate-100">"Settings"</h1>
                <p class="text-sm text-slate-400 mt-1">"Configure your node"</p>
            </div>

            // Identity Section
            <div class="bg-surface rounded-xl border border-border p-5">
                <h2 class="text-lg font-semibold text-slate-200 mb-4">"Identity"</h2>
                <div>
                    <label class="block text-sm text-slate-400 mb-1">"Peer ID"</label>
                    <div class="flex items-center gap-2">
                        <code class="flex-1 bg-surface-dark rounded-lg px-3 py-2 text-sm text-cyber-cyan font-mono break-all">
                            {move || {
                                status
                                    .get()
                                    .map(|s| s.peer_id)
                                    .unwrap_or_else(|| "Loading...".to_string())
                            }}
                        </code>
                        <button
                            on:click=copy_peer_id
                            class="shrink-0 px-3 py-2 text-sm rounded-lg bg-surface-light text-slate-300 hover:bg-slate-600 transition-colors"
                        >
                            {move || if copied.get() { "Copied!" } else { "Copy" }}
                        </button>
                    </div>
                </div>
                <div class="mt-3">
                    <label class="block text-sm text-slate-400 mb-1">"NAT Status"</label>
                    <div class="flex items-center gap-2">
                        <div class=move || {
                            let nat = status.get().map(|s| s.nat_status).unwrap_or_default();
                            match nat.as_str() {
                                "Public" => "w-2 h-2 rounded-full bg-cyber-green",
                                "Private" => "w-2 h-2 rounded-full bg-cyber-amber",
                                _ => "w-2 h-2 rounded-full bg-slate-600",
                            }
                        }></div>
                        <span class="text-sm text-slate-200">
                            {move || {
                                status
                                    .get()
                                    .map(|s| s.nat_status)
                                    .unwrap_or_else(|| "Unknown".to_string())
                            }}
                        </span>
                    </div>
                </div>
            </div>

            // Network Section
            <div class="bg-surface rounded-xl border border-border p-5">
                <h2 class="text-lg font-semibold text-slate-200 mb-4">"Network"</h2>

                // Current network status
                <div class="flex items-center gap-3 mb-4">
                    <div class=move || {
                        let mode = status.get().map(|s| s.network_mode).unwrap_or_default();
                        if mode == "private" {
                            "px-2.5 py-1 rounded-md text-xs font-medium bg-cyber-cyan/20 text-cyber-cyan"
                        } else {
                            "px-2.5 py-1 rounded-md text-xs font-medium bg-cyber-green/20 text-cyber-green"
                        }
                    }>
                        {move || {
                            let s = status.get();
                            let mode = s.as_ref().map(|s| s.network_mode.as_str()).unwrap_or("public");
                            if mode == "private" {
                                let name = s.and_then(|s| s.network_name).unwrap_or_else(|| "Private".to_string());
                                format!("Private: {name}")
                            } else {
                                "Public Network".to_string()
                            }
                        }}
                    </div>
                    {move || {
                        let is_private = status.get().map(|s| s.network_mode == "private").unwrap_or(false);
                        is_private.then(|| view! {
                            <button
                                on:click=leave_network
                                class="px-2.5 py-1 text-xs rounded-md bg-cyber-red/20 text-cyber-red hover:bg-cyber-red/30 transition-colors"
                            >
                                "Leave Network"
                            </button>
                        })
                    }}
                </div>

                // Join private network form
                <div class="border-t border-border pt-4 space-y-3">
                    <h3 class="text-sm font-medium text-slate-300">"Join Private Network"</h3>
                    <div>
                        <input
                            type="text"
                            prop:value=net_name_input
                            on:input=move |ev| set_net_name.set(event_target_value(&ev))
                            placeholder="Network name"
                            class="w-full bg-surface-dark border border-border rounded-lg px-3 py-2 text-sm text-slate-200 placeholder-slate-600 focus:border-cyber-cyan focus:outline-none"
                        />
                    </div>
                    <div>
                        <input
                            type="password"
                            prop:value=net_password_input
                            on:input=move |ev| set_net_password.set(event_target_value(&ev))
                            placeholder="Shared password"
                            class="w-full bg-surface-dark border border-border rounded-lg px-3 py-2 text-sm text-slate-200 placeholder-slate-600 focus:border-cyber-cyan focus:outline-none"
                        />
                    </div>
                    <div class="flex items-center gap-3">
                        <button
                            on:click=join_network
                            class="px-4 py-1.5 text-sm font-medium rounded-lg bg-cyber-cyan text-slate-900 hover:bg-cyan-400 transition-colors"
                        >
                            "Join"
                        </button>
                        {move || {
                            net_msg
                                .get()
                                .map(|msg| {
                                    let is_error = msg.starts_with("Error");
                                    view! {
                                        <span class=if is_error {
                                            "text-xs text-cyber-red"
                                        } else {
                                            "text-xs text-cyber-green"
                                        }>{msg}</span>
                                    }
                                })
                        }}
                    </div>
                </div>

                // Relay server address
                <div class="border-t border-border pt-4 mt-4">
                    <label class="block text-sm text-slate-400 mb-1">
                        "Relay Server Address"
                    </label>
                    <input
                        type="text"
                        prop:value=relay_input
                        on:input=move |ev| set_relay.set(event_target_value(&ev))
                        placeholder="/ip4/127.0.0.1/udp/4001/quic-v1/p2p/..."
                        class="w-full bg-surface-dark border border-border rounded-lg px-3 py-2 text-sm text-slate-200 placeholder-slate-600 focus:border-cyber-cyan focus:outline-none font-mono"
                    />
                </div>
            </div>

            // Proxy Port Section
            <div class="bg-surface rounded-xl border border-border p-5">
                <h2 class="text-lg font-semibold text-slate-200 mb-4">"Proxy Port"</h2>
                <div>
                    <label class="block text-sm text-slate-400 mb-1">"Unified Proxy Port"</label>
                    <div class="bg-surface-dark rounded-lg px-3 py-2 text-sm text-slate-300 font-mono">
                        {move || {
                            status
                                .get()
                                .and_then(|s| s.proxy_status.map(|p| p.unified_port))
                                .unwrap_or(7890)
                                .to_string()
                        }}
                    </div>
                </div>
            </div>

            // System Proxy Section
            <div class="bg-surface rounded-xl border border-border p-5">
                <div class="flex items-center justify-between">
                    <div>
                        <h2 class="text-lg font-semibold text-slate-200">"System Proxy"</h2>
                        <p class="text-sm text-slate-400 mt-0.5">
                            {move || {
                                let sys_enabled = status.get().map(|s| s.system_proxy_enabled).unwrap_or(false);
                                let proxy_running = status.get()
                                    .and_then(|s| s.proxy_status.map(|p| p.running))
                                    .unwrap_or(false);
                                if sys_enabled {
                                    "System proxy is active".to_string()
                                } else if !proxy_running {
                                    "Start proxy first".to_string()
                                } else {
                                    "Route system traffic through proxy".to_string()
                                }
                            }}
                        </p>
                    </div>
                    <button
                        on:click=move |_| {
                            let current = status.get();
                            spawn_local(async move {
                                let sys_enabled = current.as_ref().map(|s| s.system_proxy_enabled).unwrap_or(false);
                                if sys_enabled {
                                    let _ = api::clear_system_proxy().await;
                                } else {
                                    let _ = api::set_system_proxy().await;
                                }
                                if let Ok(new_status) = api::get_status().await {
                                    set_status.set(Some(new_status));
                                }
                            });
                        }
                        disabled=move || {
                            !status.get()
                                .and_then(|s| s.proxy_status.map(|p| p.running))
                                .unwrap_or(false)
                            && !status.get().map(|s| s.system_proxy_enabled).unwrap_or(false)
                        }
                        class=move || {
                            let sys_enabled = status.get().map(|s| s.system_proxy_enabled).unwrap_or(false);
                            let proxy_running = status.get()
                                .and_then(|s| s.proxy_status.map(|p| p.running))
                                .unwrap_or(false);
                            if sys_enabled {
                                "relative inline-flex h-7 w-12 items-center rounded-full bg-cyber-green transition-colors cursor-pointer"
                            } else if proxy_running {
                                "relative inline-flex h-7 w-12 items-center rounded-full bg-slate-600 transition-colors cursor-pointer"
                            } else {
                                "relative inline-flex h-7 w-12 items-center rounded-full bg-slate-700 transition-colors cursor-not-allowed opacity-50"
                            }
                        }
                    >
                        <span class=move || {
                            let sys_enabled = status.get().map(|s| s.system_proxy_enabled).unwrap_or(false);
                            if sys_enabled {
                                "inline-block h-5 w-5 transform rounded-full bg-white transition-transform translate-x-6"
                            } else {
                                "inline-block h-5 w-5 transform rounded-full bg-white transition-transform translate-x-1"
                            }
                        }></span>
                    </button>
                </div>
            </div>

            // Data Directory
            <div class="bg-surface rounded-xl border border-border p-5">
                <h2 class="text-lg font-semibold text-slate-200 mb-4">"Storage"</h2>
                <div>
                    <label class="block text-sm text-slate-400 mb-1">"Data Directory"</label>
                    <div class="bg-surface-dark rounded-lg px-3 py-2 text-sm text-slate-300 font-mono break-all">
                        {move || {
                            status
                                .get()
                                .map(|s| s.data_dir)
                                .unwrap_or_else(|| "~/.clash/".to_string())
                        }}
                    </div>
                </div>
            </div>

            // Save Button
            <div class="flex items-center gap-3">
                <button
                    on:click=save_settings
                    class="px-5 py-2 text-sm font-medium rounded-lg bg-cyber-cyan text-slate-900 hover:bg-cyan-400 transition-colors"
                >
                    "Save Settings"
                </button>
                {move || {
                    save_msg
                        .get()
                        .map(|msg| {
                            let is_error = msg.starts_with("Error");
                            view! {
                                <span class=if is_error {
                                    "text-sm text-cyber-red"
                                } else {
                                    "text-sm text-cyber-green"
                                }>{msg}</span>
                            }
                        })
                }}
            </div>
        </div>
    }
}
