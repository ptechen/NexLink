use super::sidebar::Sidebar;
use super::status_bar::StatusBar;
use leptos::prelude::*;

#[component]
pub fn Layout(children: Children) -> impl IntoView {
    view! {
        <div class="flex h-screen bg-surface-dark overflow-hidden">
            <Sidebar />
            <div class="flex-1 flex flex-col min-w-0">
                <main class="flex-1 overflow-auto p-6">{children()}</main>
                <StatusBar />
            </div>
        </div>
    }
}
