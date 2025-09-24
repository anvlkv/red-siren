use leptos::prelude::*;
use leptos_router::components::*;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::{components::Intro, routes};

#[component]
pub fn App() -> impl IntoView {
    let UseTauriWithReturn { trigger, .. } = use_command::<()>(shared::commands::health::GUI_READY);

    Effect::new(move |_| {
        log::info!("App mounted, reporting GUI ready");
        trigger(Some(()));
    });

    view! {
        <main class="bg-red dark:bg-black font-serif italic text-black dark:text-red relative h-screen w-screen">
            <div class="absolute h-full w-full overflow-hidden">
                <Intro />
            </div>
            <div class="absolute h-full w-full overflow-hidden">
                <Router>
                    <routes::AppRoutes />
                </Router>
            </div>
        </main>
    }
}
