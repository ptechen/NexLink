use crate::api;
use crate::types::TrafficStats;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn TrafficPage() -> impl IntoView {
    let (stats, set_stats) = signal(TrafficStats::default());
    let (history, set_history) = signal(Vec::<(u64, u64)>::new());

    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                if let Ok(current) = api::get_traffic().await {
                    set_history.update(|items| {
                        items.push((current.upload_speed, current.download_speed));
                        if items.len() > 60 {
                            items.remove(0);
                        }
                    });
                    set_stats.set(current);
                }
                gloo_timers::future::TimeoutFuture::new(1000).await;
            }
        });
    });

    view! {
        <div class="space-y-6">
            <div>
                <h1 class="text-2xl font-bold text-slate-100">"Traffic"</h1>
                <p class="mt-1 text-sm text-slate-400">"Real-time network statistics and quota overview"</p>
            </div>

            <div class="bg-surface rounded-2xl border border-border p-5 md:p-6">
                <div class="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
                    <div class="min-w-0">
                        <div class="flex flex-wrap items-center gap-3">
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-slate-500">
                                "Traffic Quota"
                            </p>
                            <span class=move || quota_badge_class(&stats.get())>
                                {move || quota_badge_text(&stats.get())}
                            </span>
                        </div>

                        <div class="mt-4 flex flex-wrap items-end gap-3">
                            <p class="text-3xl font-semibold text-slate-100 md:text-4xl">
                                {move || remaining_value(&stats.get())}
                            </p>
                            <span class="pb-1 text-sm text-slate-500">
                                {move || remaining_suffix(&stats.get())}
                            </span>
                        </div>

                        <p class="mt-3 max-w-2xl text-sm text-slate-400">
                            {move || quota_description(&stats.get())}
                        </p>
                    </div>

                    <div class="grid grid-cols-2 gap-3 lg:w-80">
                        <div class="rounded-xl border border-border bg-surface-dark/60 p-3">
                            <p class="text-[11px] uppercase tracking-[0.18em] text-slate-500">"Quota Total"</p>
                            <p class="mt-2 text-sm font-semibold text-slate-200">
                                {move || quota_total_value(&stats.get())}
                            </p>
                        </div>
                        <div class="rounded-xl border border-border bg-surface-dark/60 p-3">
                            <p class="text-[11px] uppercase tracking-[0.18em] text-slate-500">"Relay Used"</p>
                            <p class="mt-2 text-sm font-semibold text-slate-200">
                                {move || quota_used_value(&stats.get())}
                            </p>
                        </div>
                        <div class="rounded-xl border border-border bg-surface-dark/60 p-3">
                            <p class="text-[11px] uppercase tracking-[0.18em] text-slate-500">"Session Total"</p>
                            <p class="mt-2 text-sm font-semibold text-slate-200">
                                {move || format_bytes(session_total(&stats.get()))}
                            </p>
                        </div>
                        <div class="rounded-xl border border-border bg-surface-dark/60 p-3">
                            <p class="text-[11px] uppercase tracking-[0.18em] text-slate-500">"Usage"</p>
                            <p class="mt-2 text-sm font-semibold text-slate-200">
                                {move || quota_usage_value(&stats.get())}
                            </p>
                        </div>
                    </div>
                </div>

                <div class="mt-5">
                    <div class="h-2 overflow-hidden rounded-full bg-surface-dark">
                        <div
                            class=move || quota_progress_class(&stats.get())
                            style=move || quota_progress_style(&stats.get())
                        ></div>
                    </div>
                    <div class="mt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-slate-500">
                        <span>{move || quota_progress_label(&stats.get())}</span>
                        <span>{move || quota_progress_hint(&stats.get())}</span>
                    </div>
                </div>
            </div>

            <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
                <div class="bg-surface rounded-xl border border-border p-5">
                    <div class="mb-3 flex items-center gap-2">
                        <span class="text-lg text-cyber-cyan">"\u{2191}"</span>
                        <span class="text-sm text-slate-400">"Upload"</span>
                    </div>
                    <p class="text-2xl font-bold text-slate-100">
                        {move || format_speed(stats.get().upload_speed)}
                    </p>
                    <p class="mt-1 text-xs text-slate-500">
                        "Session total: " {move || format_bytes(stats.get().bytes_sent)}
                    </p>
                </div>

                <div class="bg-surface rounded-xl border border-border p-5">
                    <div class="mb-3 flex items-center gap-2">
                        <span class="text-lg text-cyber-green">"\u{2193}"</span>
                        <span class="text-sm text-slate-400">"Download"</span>
                    </div>
                    <p class="text-2xl font-bold text-slate-100">
                        {move || format_speed(stats.get().download_speed)}
                    </p>
                    <p class="mt-1 text-xs text-slate-500">
                        "Session total: " {move || format_bytes(stats.get().bytes_received)}
                    </p>
                </div>
            </div>

            <div class="bg-surface rounded-xl border border-border p-5">
                <h2 class="mb-4 text-sm font-medium text-slate-400">"Speed History (60s)"</h2>
                <div class="flex h-32 items-end gap-px">
                    {move || {
                        let items = history.get();
                        let max_speed = items
                            .iter()
                            .flat_map(|(up, down)| [*up, *down])
                            .max()
                            .unwrap_or(1)
                            .max(1);

                        items
                            .iter()
                            .map(|(up, down)| {
                                let up_pct = (*up as f64 / max_speed as f64 * 100.0) as u32;
                                let down_pct = (*down as f64 / max_speed as f64 * 100.0) as u32;
                                let up_height = format!("height: {}%", up_pct.max(1));
                                let down_height = format!("height: {}%", down_pct.max(1));

                                view! {
                                    <div class="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-px">
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

                <div class="mt-2 flex justify-between">
                    <div class="flex items-center gap-2">
                        <div class="h-2 w-3 rounded-sm bg-cyber-cyan/60"></div>
                        <span class="text-xs text-slate-500">"Upload"</span>
                    </div>
                    <div class="flex items-center gap-2">
                        <div class="h-2 w-3 rounded-sm bg-cyber-green/60"></div>
                        <span class="text-xs text-slate-500">"Download"</span>
                    </div>
                </div>
            </div>

            <div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
                <div class="bg-surface rounded-xl border border-border p-4">
                    <p class="mb-1 text-xs text-slate-500">"Active Connections"</p>
                    <p class="text-lg font-semibold text-slate-200">
                        {move || stats.get().active_connections.to_string()}
                    </p>
                </div>
                <div class="bg-surface rounded-xl border border-border p-4">
                    <p class="mb-1 text-xs text-slate-500">"Session Sent"</p>
                    <p class="text-lg font-semibold text-slate-200">
                        {move || format_bytes(stats.get().bytes_sent)}
                    </p>
                </div>
                <div class="bg-surface rounded-xl border border-border p-4">
                    <p class="mb-1 text-xs text-slate-500">"Session Received"</p>
                    <p class="text-lg font-semibold text-slate-200">
                        {move || format_bytes(stats.get().bytes_received)}
                    </p>
                </div>
                <div class="bg-surface rounded-xl border border-border p-4">
                    <p class="mb-1 text-xs text-slate-500">"Relay Total Used"</p>
                    <p class="text-lg font-semibold text-slate-200">
                        {move || quota_used_value(&stats.get())}
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
        format!("{size:.1} {}", units[unit_idx])
    }
}

fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec))
}

fn session_total(stats: &TrafficStats) -> u64 {
    stats.bytes_sent.saturating_add(stats.bytes_received)
}

fn quota_badge_class(stats: &TrafficStats) -> &'static str {
    if !stats.quota_available {
        "rounded-full border border-slate-700 bg-slate-800/80 px-2.5 py-1 text-xs font-medium text-slate-400"
    } else if stats.total_limit == 0 {
        "rounded-full border border-cyber-cyan/30 bg-cyber-cyan/10 px-2.5 py-1 text-xs font-medium text-cyber-cyan"
    } else if stats.usage_ratio >= 0.9 {
        "rounded-full border border-cyber-red/30 bg-cyber-red/10 px-2.5 py-1 text-xs font-medium text-cyber-red"
    } else if stats.usage_ratio >= 0.75 {
        "rounded-full border border-cyber-amber/30 bg-cyber-amber/10 px-2.5 py-1 text-xs font-medium text-cyber-amber"
    } else {
        "rounded-full border border-cyber-green/30 bg-cyber-green/10 px-2.5 py-1 text-xs font-medium text-cyber-green"
    }
}

fn quota_badge_text(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "Unavailable".to_string()
    } else if stats.total_limit == 0 {
        "Unlimited".to_string()
    } else if stats.usage_ratio >= 0.9 {
        "Low Remaining".to_string()
    } else if stats.usage_ratio >= 0.75 {
        "Watch Usage".to_string()
    } else {
        "Healthy".to_string()
    }
}

fn remaining_value(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "--".to_string()
    } else if stats.total_limit == 0 {
        "Unlimited".to_string()
    } else {
        format_bytes(stats.remaining_bytes)
    }
}

fn remaining_suffix(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "Quota data is not available".to_string()
    } else if stats.total_limit == 0 {
        "No traffic cap configured".to_string()
    } else {
        "remaining".to_string()
    }
}

fn quota_description(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "The app could not load quota data for this peer yet. Session counters below are still live."
            .to_string()
    } else if stats.total_limit == 0 {
        "This peer does not currently have a capped traffic package, so only live session traffic is tracked."
            .to_string()
    } else {
        format!(
            "{} of {} has been reported by the relay for your account usage.",
            format_bytes(stats.total_used),
            format_bytes(stats.total_limit)
        )
    }
}

fn quota_total_value(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "--".to_string()
    } else if stats.total_limit == 0 {
        "Unlimited".to_string()
    } else {
        format_bytes(stats.total_limit)
    }
}

fn quota_used_value(stats: &TrafficStats) -> String {
    if stats.quota_available {
        format_bytes(stats.total_used)
    } else {
        "--".to_string()
    }
}

fn quota_usage_value(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "--".to_string()
    } else if stats.total_limit == 0 {
        "0%".to_string()
    } else {
        format!("{:.0}%", stats.usage_ratio.clamp(0.0, 1.0) * 100.0)
    }
}

fn quota_progress_class(stats: &TrafficStats) -> &'static str {
    if !stats.quota_available {
        "h-full rounded-full bg-slate-700 transition-all duration-300"
    } else if stats.total_limit == 0 {
        "h-full rounded-full bg-cyber-cyan/70 transition-all duration-300"
    } else if stats.usage_ratio >= 0.9 {
        "h-full rounded-full bg-cyber-red transition-all duration-300"
    } else if stats.usage_ratio >= 0.75 {
        "h-full rounded-full bg-cyber-amber transition-all duration-300"
    } else {
        "h-full rounded-full bg-cyber-green transition-all duration-300"
    }
}

fn quota_progress_style(stats: &TrafficStats) -> String {
    let width = if !stats.quota_available {
        0.0
    } else if stats.total_limit == 0 {
        100.0
    } else {
        stats.usage_ratio.clamp(0.0, 1.0) * 100.0
    };
    format!("width: {width:.2}%")
}

fn quota_progress_label(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "Quota source unavailable".to_string()
    } else if stats.total_limit == 0 {
        "Unlimited plan".to_string()
    } else {
        format!(
            "{} used / {} total",
            format_bytes(stats.total_used),
            format_bytes(stats.total_limit)
        )
    }
}

fn quota_progress_hint(stats: &TrafficStats) -> String {
    if !stats.quota_available {
        "Live speed cards still update every second".to_string()
    } else if stats.total_limit == 0 {
        format!("Session traffic: {}", format_bytes(session_total(stats)))
    } else {
        format!("{} remaining", format_bytes(stats.remaining_bytes))
    }
}
