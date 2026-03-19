use crate::api;
use crate::types::{AppStatus, PeerInfo};
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn NodesPage() -> impl IntoView {
    let (status, set_status) = signal(Option::<AppStatus>::None);
    let (error_msg, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(true);

    // Fetch status on mount
    Effect::new(move |_| {
        spawn_local(async move {
            match api::get_status().await {
                Ok(s) => {
                    set_status.set(Some(s));
                    set_loading.set(false);
                }
                Err(e) => {
                    set_error.set(Some(e));
                    set_loading.set(false);
                }
            }
        });
    });

    let toggle_proxy = move |_| {
        spawn_local(async move {
            let running = api::get_status()
                .await
                .ok()
                .and_then(|s| s.proxy_status.map(|p| p.running))
                .unwrap_or(false);
            if running {
                let _ = api::stop_proxy().await;
            } else {
                if api::start_proxy(7890).await.is_ok() {
                    let _ = api::set_system_proxy().await;
                }
            }
            if let Ok(new_status) = api::get_status().await {
                set_status.set(Some(new_status));
            }
        });
    };

    let disconnect = move |_| {
        spawn_local(async move {
            let _ = api::disconnect_node().await;
            if let Ok(new_status) = api::get_status().await {
                set_status.set(Some(new_status));
            }
        });
    };

    let refresh_nodes = move |_| {
        set_loading.set(true);
        spawn_local(async move {
            let _ = api::list_nodes().await;
            if let Ok(new_status) = api::get_status().await {
                set_status.set(Some(new_status));
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="space-y-6">
            // Header
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-bold text-slate-100">"Nodes"</h1>
                    <p class="text-sm text-slate-400 mt-1">"Manage peer connections and proxy"</p>
                </div>
                <button
                    on:click=refresh_nodes
                    class="px-3 py-1.5 text-sm rounded-lg bg-surface-light text-slate-300 hover:bg-slate-600 transition-colors"
                >
                    "Refresh"
                </button>
            </div>

            // Proxy Toggle Card
            <div class="bg-surface rounded-xl border border-border p-5">
                <div class="flex items-center justify-between">
                    <div>
                        <h2 class="text-lg font-semibold text-slate-200">"Proxy"</h2>
                        <p class="text-sm text-slate-400 mt-0.5">
                            {move || {
                                let proxy = status
                                    .get()
                                    .map(|s| (s.proxy_status, s.system_proxy_enabled));
                                match proxy {
                                    Some((Some(p), true)) if p.running =>
                                        format!("UNIFIED :{} | System Proxy Active", p.unified_port),
                                    Some((Some(p), false)) if p.running =>
                                        format!("UNIFIED :{} | System Proxy Inactive", p.unified_port),
                                    Some((Some(_), _)) => "Stopped".to_string(),
                                    Some((None, _)) => "Not initialized".to_string(),
                                    None => "Not initialized".to_string(),
                                }
                            }}
                        </p>
                    </div>
                    <button
                        on:click=toggle_proxy
                        class=move || {
                            let running = status
                                .get()
                                .and_then(|s| s.proxy_status.map(|p| p.running))
                                .unwrap_or(false);
                            if running {
                                "relative inline-flex h-7 w-12 items-center rounded-full bg-cyber-green transition-colors cursor-pointer"
                            } else {
                                "relative inline-flex h-7 w-12 items-center rounded-full bg-slate-600 transition-colors cursor-pointer"
                            }
                        }
                    >
                        <span class=move || {
                            let running = status
                                .get()
                                .and_then(|s| s.proxy_status.map(|p| p.running))
                                .unwrap_or(false);
                            if running {
                                "inline-block h-5 w-5 transform rounded-full bg-white transition-transform translate-x-6"
                            } else {
                                "inline-block h-5 w-5 transform rounded-full bg-white transition-transform translate-x-1"
                            }
                        }></span>
                    </button>
                </div>
            </div>

            // Connected Peer
            {move || {
                status
                    .get()
                    .and_then(|s| {
                        s.connected_peer
                            .map(|peer| {
                                view! {
                                    <div class="bg-surface rounded-xl border border-cyber-cyan/30 p-5">
                                        <div class="flex items-center justify-between">
                                            <div class="flex items-center gap-3">
                                                <div class="w-2.5 h-2.5 rounded-full bg-cyber-green animate-pulse"></div>
                                                <div>
                                                    <p class="text-sm font-medium text-slate-200">
                                                        "Connected to"
                                                    </p>
                                                    <p class="text-xs text-cyber-cyan font-mono mt-0.5">
                                                        {truncate_peer_id(&peer)}
                                                    </p>
                                                </div>
                                            </div>
                                            <button
                                                on:click=disconnect
                                                class="px-3 py-1.5 text-sm rounded-lg bg-cyber-red/10 text-cyber-red border border-cyber-red/20 hover:bg-cyber-red/20 transition-colors"
                                            >
                                                "Disconnect"
                                            </button>
                                        </div>
                                    </div>
                                }
                            })
                    })
            }}

            // Node List
            <div>
                <h2 class="text-lg font-semibold text-slate-200 mb-3">"Discovered Peers"</h2>
                {move || {
                    if loading.get() {
                        view! {
                            <div class="flex items-center justify-center py-12">
                                <div class="w-8 h-8 border-2 border-cyber-cyan/30 border-t-cyber-cyan rounded-full animate-spin"></div>
                            </div>
                        }
                            .into_any()
                    } else {
                        let peers = status
                            .get()
                            .map(|s| s.discovered_peers)
                            .unwrap_or_default();
                        if peers.is_empty() {
                            view! {
                                <div class="text-center py-12 bg-surface rounded-xl border border-border">
                                    <p class="text-slate-500 text-sm">"No peers discovered yet"</p>
                                    <p class="text-slate-600 text-xs mt-1">
                                        "Make sure the relay server is running"
                                    </p>
                                </div>
                            }
                                .into_any()
                        } else {
                            let set_status = set_status.clone();
                            view! {
                                <div class="space-y-2">
                                    {peers
                                        .into_iter()
                                        .map(|peer| {
                                            let peer_id_for_click = peer.peer_id.clone();
                                            let set_status = set_status.clone();
                                            view! {
                                                <PeerCard
                                                    peer=peer
                                                    on_connect=move |_| {
                                                        let pid = peer_id_for_click.clone();
                                                        let set_status = set_status.clone();
                                                        spawn_local(async move {
                                                            let _ = api::connect_node(&pid).await;
                                                            if let Ok(new_status) = api::get_status().await {
                                                                set_status.set(Some(new_status));
                                                            }
                                                        });
                                                    }
                                                />
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }
                    }
                }}
            </div>

            // Error display
            {move || {
                error_msg
                    .get()
                    .map(|e| {
                        view! {
                            <div class="bg-cyber-red/10 border border-cyber-red/20 rounded-lg p-3">
                                <p class="text-sm text-cyber-red">{e}</p>
                            </div>
                        }
                    })
            }}
        </div>
    }
}

#[component]
fn PeerCard(peer: PeerInfo, on_connect: impl Fn(()) + 'static) -> impl IntoView {
    let display_id = truncate_peer_id(&peer.peer_id);
    let latency_text = peer
        .latency_ms
        .map(|ms| format!("{ms}ms"))
        .unwrap_or_else(|| "--".to_string());
    let is_connected = peer.connected;
    let is_provider = peer.is_provider;

    view! {
        <div class="bg-surface rounded-xl border border-border p-4 hover:border-slate-600 transition-colors">
            <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                    <div class=if is_connected {
                        "w-2.5 h-2.5 rounded-full bg-cyber-green"
                    } else {
                        "w-2.5 h-2.5 rounded-full bg-slate-600"
                    }></div>
                    <div>
                        <p class="text-sm font-mono text-slate-200">{display_id}</p>
                        <div class="flex items-center gap-2 mt-0.5">
                            {is_provider
                                .then(|| {
                                    view! {
                                        <span class="text-xs px-1.5 py-0.5 rounded bg-cyber-cyan/10 text-cyber-cyan">
                                            "Provider"
                                        </span>
                                    }
                                })}
                            {is_connected
                                .then(|| {
                                    view! {
                                        <span class="text-xs px-1.5 py-0.5 rounded bg-cyber-green/10 text-cyber-green">
                                            "Best"
                                        </span>
                                    }
                                })}
                            <span class=if peer.latency_ms.map(|ms| ms < 100).unwrap_or(false) {
                                "text-xs text-cyber-green"
                            } else if peer.latency_ms.map(|ms| ms < 300).unwrap_or(false) {
                                "text-xs text-yellow-400"
                            } else {
                                "text-xs text-slate-500"
                            }>{latency_text}</span>
                        </div>
                    </div>
                </div>
                {(!is_connected)
                    .then(|| {
                        view! {
                            <button
                                on:click=move |_| on_connect(())
                                class="px-3 py-1.5 text-sm rounded-lg bg-cyber-cyan/10 text-cyber-cyan border border-cyber-cyan/20 hover:bg-cyber-cyan/20 transition-colors"
                            >
                                "Connect"
                            </button>
                        }
                    })}
            </div>
        </div>
    }
}

fn truncate_peer_id(peer_id: &str) -> String {
    if peer_id.len() > 16 {
        format!("{}...{}", &peer_id[..8], &peer_id[peer_id.len() - 6..])
    } else {
        peer_id.to_string()
    }
}
