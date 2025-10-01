use leptos::prelude::*;
use leptos_router::components::*;
use leptos_use::{signal_debounced, use_preferred_dark, use_window_size, UseWindowSizeReturn};
use shared::commands::setup::{UpdateWindowAppearancePayload, UpdateWindowSizePayload};
use tauri_use::{
    use_command, use_invoke, use_listen, EventType, UseListenReturn, UseTauriReturn,
    UseTauriWithReturn,
};

use crate::{
    routes::AppRoutes,
    util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};

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

    let UseTauriResourceReturn {
        data: window_appearance_override,
        ..
    } = use_tauri_resource::<shared::commands::setup::UpdateWindowAppearanceOverridePayload>(
        shared::commands::setup::GET_WINDOW_APPEARANCE_OVERRIDE,
    );

    let preferred_dark = use_preferred_dark();
    let UseWindowSizeReturn { width, height } = use_window_size();
    let width = signal_debounced(width, 70.0);
    let height = signal_debounced(height, 70.0);

    let window_appearance_class = Signal::derive(move || {
        let os_theme = preferred_dark();
        let user_theme = window_appearance_override().and_then(|t| t.dark);
        let is_dark = user_theme.unwrap_or(os_theme);

        if is_dark {
            "dark"
        } else {
            "light"
        }
    });

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
        let width = width();
        let height = height();
        log::info!("Detected window resize: {width}, {height}");
        trigger_update_window_size(Some((UpdateWindowSizePayload { width, height }, ())));
    });

    on_cleanup(move || {
        close_app_ready();
    });

    view! {
        <main class=format!(
            "bg-red dark:bg-black font-serif text-black dark:text-red relative h-screen w-screen {}",
            window_appearance_class(),
        )>
            <Router>
                <AppRoutes />
            </Router>
        </main>
    }
}
