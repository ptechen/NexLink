use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{components::*, path};

use crate::components::layout::Layout;
use crate::pages::nodes::NodesPage;
use crate::pages::settings::SettingsPage;
use crate::pages::traffic::TrafficPage;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="NexLink P2P" />
        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />

        <Router>
            <Layout>
                <Routes fallback=|| {
                    view! {
                        <div class="flex items-center justify-center h-full">
                            <p class="text-slate-400 text-lg">"Page not found"</p>
                        </div>
                    }
                }>
                    <Route path=path!("/") view=NodesPage />
                    <Route path=path!("/traffic") view=TrafficPage />
                    <Route path=path!("/settings") view=SettingsPage />
                </Routes>
            </Layout>
        </Router>
    }
}
