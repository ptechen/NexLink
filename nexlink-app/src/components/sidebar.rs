use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

#[component]
pub fn Sidebar() -> impl IntoView {
    let location = use_location();

    let nav_items: Vec<(&str, &str, &str)> = vec![
        ("/", "Nodes", SVG_NODES),
        ("/traffic", "Traffic", SVG_TRAFFIC),
        ("/settings", "Settings", SVG_SETTINGS),
    ];

    view! {
        <nav class="w-56 bg-surface border-r border-border flex flex-col h-full shrink-0">
            // Logo
            <div class="px-4 py-5 border-b border-border">
                <h1 class="text-lg font-bold text-cyber-cyan tracking-wider">"CLASH P2P"</h1>
                <p class="text-xs text-slate-500 mt-0.5">"Decentralized Proxy"</p>
            </div>

            // Navigation
            <div class="flex-1 py-3">
                {nav_items
                    .into_iter()
                    .map(|(href, label, icon)| {
                        let href_clone = href.to_string();
                        let is_active = {
                            let href_clone2 = href_clone.clone();
                            move || {
                                let path = location.pathname.get();
                                if href_clone2 == "/" { path == "/" } else { path.starts_with(&href_clone2) }
                            }
                        };
                        let icon = icon.to_string();

                        view! {
                            <A
                                href=href
                                attr:class=move || {
                                    let base = "flex items-center gap-3 px-4 py-2.5 mx-2 rounded-lg text-sm transition-all duration-200";
                                    if is_active() {
                                        format!(
                                            "{base} bg-cyber-cyan/10 text-cyber-cyan border border-cyber-cyan/20",
                                        )
                                    } else {
                                        format!(
                                            "{base} text-slate-400 hover:text-slate-200 hover:bg-slate-800",
                                        )
                                    }
                                }
                            >

                                <span inner_html=icon.clone()></span>
                                <span>{label}</span>
                            </A>
                        }
                    })
                    .collect_view()}
            </div>

            // Version
            <div class="px-4 py-3 border-t border-border">
                <p class="text-xs text-slate-600">"v0.1.0"</p>
            </div>
        </nav>
    }
}

const SVG_NODES: &str = r#"<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><circle cx="5" cy="12" r="2" stroke-width="1.5"/><circle cx="19" cy="12" r="2" stroke-width="1.5"/><circle cx="12" cy="5" r="2" stroke-width="1.5"/><circle cx="12" cy="19" r="2" stroke-width="1.5"/><path d="M6.5 10.5L10.5 6.5M17.5 10.5L13.5 6.5M6.5 13.5L10.5 17.5M17.5 13.5L13.5 17.5" stroke-width="1.5" stroke-linecap="round"/><path d="M7 12H10M14 12H17M12 7V10M12 14V17" stroke-width="1.5" stroke-linecap="round"/></svg>"#;

const SVG_TRAFFIC: &str = r#"<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path d="M13 2L3 14H12L11 22L21 10H12L13 2Z" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M3 14L21 10" stroke-width="1.5" stroke-linecap="round"/><path d="M12 22L11 14" stroke-width="1.5" stroke-linecap="round"/></svg>"#;

const SVG_SETTINGS: &str = r#"<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path d="M12 15C13.6569 15 15 13.6569 15 12C15 10.3431 13.6569 9 12 9C10.3431 9 9 10.3431 9 12C9 13.6569 10.3431 15 12 15Z" stroke-width="1.5"/><path d="M19.4 15C19.7314 15.6627 19.9758 16.3627 20.1213 17H22V19H20.1213C19.9758 19.6373 19.7314 20.3373 19.4 21H17.6C17.2686 20.3373 17.0242 19.6373 16.8787 19H15.1213C14.9758 19.6373 14.7314 20.3373 14.4 21H12.6C12.2686 20.3373 12.0242 19.6373 11.8787 19H10.1213C9.97581 19.6373 9.7314 20.3373 9.4 21H7.6C7.2686 20.3373 7.02419 19.6373 6.87868 19H5V17H6.87868C7.02419 16.3627 7.2686 15.6627 7.6 15H9.4C9.7314 15.6627 9.97581 16.3627 10.1213 17H11.8787C12.0242 16.3627 12.2686 15.6627 12.6 15H14.4C14.7314 15.6627 14.9758 16.3627 15.1213 17H16.8787C17.0242 16.3627 17.2686 15.6627 17.6 15H19.4Z" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
