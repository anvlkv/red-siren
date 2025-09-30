use leptos::prelude::*;
use leptos_router::components::*;
use leptos_use::{use_preferred_dark, use_window_size};
use shared::commands::setup::{UpdateWindowAppearancePayload, UpdateWindowSizePayload};
use tauri_use::{
    use_command, use_invoke, use_listen, EventType, UseListenReturn, UseTauriReturn,
    UseTauriWithReturn,
};

use crate::routes::AppRoutes;

#[component]
pub fn App() -> impl IntoView {
    let UseTauriWithReturn {
        trigger: trigger_gui_ready,
        ..
    } = use_command::<()>(shared::commands::health::GUI_READY);

    let UseListenReturn {
        event_id: app_ready,
        open,
        error,
        close: close_app_ready,
        ..
    } = use_listen::<()>(EventType::Custom(shared::events::health::APP_READY));

    let UseTauriWithReturn {
        error: nav_bootstrap_error,
        trigger: bootstrap_trigger,
        ..
    } = use_command::<()>(shared::commands::navigation::NAV_BOOTSTRAP);

    let UseTauriReturn {
        trigger: trigger_update_window_appearance,
        error: error_update_window_appearance,
        ..
    } = use_invoke::<UpdateWindowAppearancePayload, (), ()>(
        shared::commands::setup::UPDATE_WINDOW_APPEARANCE,
    );

    let UseTauriReturn {
        trigger: trigger_update_window_size,
        error: error_update_window_size,
        ..
    } = use_invoke::<UpdateWindowSizePayload, (), ()>(shared::commands::setup::UPDATE_WINDOW_SIZE);

    let size = use_window_size();
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
        if let Some(err) = error_update_window_appearance() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::setup::UPDATE_WINDOW_APPEARANCE
            );
        }
        if let Some(err) = error_update_window_size() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::setup::UPDATE_WINDOW_SIZE
            );
        }
    });

    Effect::new(move |_| {
        if app_ready().is_some() {
            bootstrap_trigger(Some(()));
        }
    });

    Effect::new(move |_| {
        let dark = preferred_dark();
        log::info!("Preferred dark mode: {dark}",);
        trigger_update_window_appearance(Some((UpdateWindowAppearancePayload { dark }, ())));
    });

    Effect::new(move |_| {
        let width = size.width.get();
        let height = size.height.get();
        log::info!("Window size: {width}, {height}");
        trigger_update_window_size(Some((UpdateWindowSizePayload { width, height }, ())));
    });

    on_cleanup(move || {
        close_app_ready();
    });

    view! {
        <main class="bg-red dark:bg-black font-serif text-black dark:text-red relative h-screen w-screen">
            <Router>
                <AppRoutes />
            </Router>
        </main>
    }
}
