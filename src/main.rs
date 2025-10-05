mod app;
mod components;
mod pages;
mod routes;
mod util;

use app::*;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();

    util::log::init_tauri_logger();

    log::info!("Mounting app");

    leptos_styling::init();

    mount_to_body(|| {
        view! { <App /> }
    })
}
