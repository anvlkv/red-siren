use leptos::prelude::*;
use leptos_router::components::*;
use leptos_use::use_preferred_dark;
use shared::commands::setup::UpdateWindowAppearancePayload;
use tauri_use::{
    use_command, use_invoke, use_listen, EventType, UseListenReturn, UseTauriReturn,
    UseTauriWithReturn,
};

use crate::{components::Intro, routes};

#[component]
pub fn App() -> impl IntoView {
    let UseTauriWithReturn {
        trigger: trigger_gui_ready,
        ..
    } = use_command::<()>(shared::commands::health::GUI_READY);

    let UseListenReturn {
        event_id,
        open,
        error,
        ..
    } = use_listen::<()>(EventType::Custom(shared::events::health::APP_READY));

    let UseTauriWithReturn {
        error: nav_bootstrap_error,
        trigger: bootstrap_trigger,
        ..
    } = use_command::<()>(shared::commands::navigation::NAV_BOOTSTRAP);

    let UseTauriReturn {
        trigger: trigger_update_window_appearance,
        ..
    } = use_invoke::<UpdateWindowAppearancePayload, (), ()>(
        shared::commands::setup::UPDATE_WINDOW_APPEARANCE,
    );

    let preferred_dark = use_preferred_dark();

    Effect::new(move |_| {
        open();

        log::info!("App mounted, reporting GUI ready");
        trigger_gui_ready(Some(()));
    });

    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::health::APP_READY
            )
        }

        if let Some(err) = nav_bootstrap_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::navigation::NAV_BOOTSTRAP
            );
        }
    });

    Effect::new(move |_| {
        if event_id().is_some() {
            bootstrap_trigger(Some(()));
        }
    });

    Effect::new(move |_| {
        let dark = preferred_dark();
        log::info!("Preferred dark mode: {}", dark);
        trigger_update_window_appearance(Some((UpdateWindowAppearancePayload { dark }, ())));
    });

    view! {
        <main class="bg-red dark:bg-black font-serif text-black dark:text-red relative h-screen w-screen">
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
