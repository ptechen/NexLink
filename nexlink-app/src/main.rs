pub mod api;
pub mod app;
pub mod components;
pub mod pages;
pub mod types;

use app::App;
use leptos::prelude::*;

fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    mount_to_body(App);
}
