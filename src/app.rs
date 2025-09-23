use leptos::prelude::*;
use leptos_router::components::*;
use tauri_use::{use_invoke, UseTauriReturn};

use crate::routes;

#[component]
pub fn App() -> impl IntoView {
    let UseTauriReturn { trigger, .. } =
        use_invoke::<(), (), ()>(shared::commands::health::GUI_READY);

    Effect::new(move |_| {
        trigger(Some(((), ())));
    });

    view! {
        <main class="bg-red dark:bg-black font-serif italic">
            <Router>
                <routes::AppRoutes />
            </Router>
        </main>
    }
}
