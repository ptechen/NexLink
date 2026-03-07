use crate::api;
use crate::types::TrafficStats;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn TrafficPage() -> impl IntoView {
    let (stats, set_stats) = signal(TrafficStats::default());
    let (history, set_history) = signal(Vec::<(u64, u64)>::new());

    // Poll traffic stats every second
    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                if let Ok(s) = api::get_traffic().await {
                    set_history.update(|h| {
                        h.push((s.upload_speed, s.download_speed));
                        if h.len() > 60 {
                            h.remove(0);
                        }
                    });
                    set_stats.set(s);
                }
                gloo_timers::future::TimeoutFuture::new(1000).await;
            }
        });
    });

    view! {
        <div class="space-y-6">
            <div>
                <h1 class="text-2xl font-bold text-slate-100">"Traffic"</h1>
                <p class="text-sm text-slate-400 mt-1">"Real-time network statistics"</p>
            </div>

            // Speed Cards
            <div class="grid grid-cols-2 gap-4">
                <div class="bg-surface rounded-xl border border-border p-5">
                    <div class="flex items-center gap-2 mb-3">
                        <span class="text-cyber-cyan text-lg">"\u{2191}"</span>
                        <span class="text-sm text-slate-400">"Upload"</span>
                    </div>
                    <p class="text-2xl font-bold text-slate-100">
                        {move || format_speed(stats.get().upload_speed)}
                    </p>
                    <p class="text-xs text-slate-500 mt-1">
                        "Total: " {move || format_bytes(stats.get().bytes_sent)}
                    </p>
                </div>
                <div class="bg-surface rounded-xl border border-border p-5">
                    <div class="flex items-center gap-2 mb-3">
                        <span class="text-cyber-green text-lg">"\u{2193}"</span>
                        <span class="text-sm text-slate-400">"Download"</span>
                    </div>
                    <p class="text-2xl font-bold text-slate-100">
                        {move || format_speed(stats.get().download_speed)}
                    </p>
                    <p class="text-xs text-slate-500 mt-1">
                        "Total: " {move || format_bytes(stats.get().bytes_received)}
                    </p>
                </div>
            </div>

            // Traffic Graph
            <div class="bg-surface rounded-xl border border-border p-5">
                <h2 class="text-sm font-medium text-slate-400 mb-4">"Speed History (60s)"</h2>
                <div class="h-32 flex items-end gap-px">
                    {move || {
                        let h = history.get();
                        let max_speed = h
                            .iter()
                            .flat_map(|(u, d)| [*u, *d])
                            .max()
                            .unwrap_or(1)
                            .max(1);
                        h.iter()
                            .map(|(up, down)| {
                                let up_pct = (*up as f64 / max_speed as f64 * 100.0) as u32;
                                let down_pct = (*down as f64 / max_speed as f64 * 100.0) as u32;
                                let up_height = format!("height: {}%", up_pct.max(1));
                                let down_height = format!("height: {}%", down_pct.max(1));
                                view! {
                                    <div class="flex-1 flex flex-col gap-px items-center justify-end h-full min-w-0">
                                        <div
                                            class="w-full rounded-t-sm bg-cyber-cyan/60 transition-all duration-300"
                                            style=up_height
                                        ></div>
                                        <div
                                            class="w-full rounded-b-sm bg-cyber-green/60 transition-all duration-300"
                                            style=down_height
                                        ></div>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
                <div class="flex justify-between mt-2">
                    <div class="flex items-center gap-2">
                        <div class="w-3 h-2 rounded-sm bg-cyber-cyan/60"></div>
                        <span class="text-xs text-slate-500">"Upload"</span>
                    </div>
                    <div class="flex items-center gap-2">
                        <div class="w-3 h-2 rounded-sm bg-cyber-green/60"></div>
                        <span class="text-xs text-slate-500">"Download"</span>
                    </div>
                </div>
            </div>

            // Stats Grid
            <div class="grid grid-cols-3 gap-4">
                <div class="bg-surface rounded-xl border border-border p-4">
                    <p class="text-xs text-slate-500 mb-1">"Active Connections"</p>
                    <p class="text-lg font-semibold text-slate-200">
                        {move || stats.get().active_connections.to_string()}
                    </p>
                </div>
                <div class="bg-surface rounded-xl border border-border p-4">
                    <p class="text-xs text-slate-500 mb-1">"Total Sent"</p>
                    <p class="text-lg font-semibold text-slate-200">
                        {move || format_bytes(stats.get().bytes_sent)}
                    </p>
                </div>
                <div class="bg-surface rounded-xl border border-border p-4">
                    <p class="text-xs text-slate-500 mb-1">"Total Received"</p>
                    <p class="text-lg font-semibold text-slate-200">
                        {move || format_bytes(stats.get().bytes_received)}
                    </p>
                </div>
            </div>
        </div>
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", size, units[unit_idx])
    }
}

fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec))
}
