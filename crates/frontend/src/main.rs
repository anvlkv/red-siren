mod app;
mod pages;
mod routes;

use app::App;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();

    leptos_styling::init();

    mount_to_body(|| {
        view! { <App /> }
    })
}
