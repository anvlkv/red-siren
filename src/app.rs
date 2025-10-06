use common::commands::setup::{UpdateWindowAppearancePayload, UpdateWindowSizePayload};
use leptos::prelude::*;
use leptos_router::components::*;
use leptos_use::{signal_debounced, use_preferred_dark, use_window_size, UseWindowSizeReturn};
use tauri_use::{
    use_command, use_invoke, use_listen, EventType, UseListenReturn, UseTauriReturn,
    UseTauriWithReturn,
};

use crate::{
    routes::AppRoutes,
    util::layout_context::provide_layout_context,
    util::playback_service::provide_playback_service,
    util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};

#[component]
pub fn App() -> impl IntoView {
    let UseTauriWithReturn {
        trigger: trigger_gui_ready,
        ..
    } = use_command::<()>(common::commands::health::GUI_READY);

    let UseListenReturn {
        event_id: _app_ready,
        open,
        error,
        close: close_app_ready,
        ..
    } = use_listen::<()>(EventType::Custom(common::events::health::APP_READY));

    let UseTauriReturn {
        trigger: trigger_update_window_appearance,
        error: error_update_window_appearance,
        ..
    } = use_invoke::<UpdateWindowAppearancePayload, (), ()>(
        common::commands::setup::UPDATE_WINDOW_APPEARANCE,
    );

    let UseTauriReturn {
        trigger: trigger_update_window_size,
        error: error_update_window_size,
        ..
    } = use_invoke::<UpdateWindowSizePayload, (), ()>(common::commands::setup::UPDATE_WINDOW_SIZE);

    let UseTauriResourceReturn {
        data: window_appearance_override,
        ..
    } = use_tauri_resource::<common::commands::setup::UpdateWindowAppearanceOverridePayload>(
        common::commands::setup::GET_WINDOW_APPEARANCE_OVERRIDE,
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
            "contents dark"
        } else {
            "contents light"
        }
    });

    crate::util::view_transitions::provide_view_transition_class_toggler();

    Effect::new(move |_| {
        open();

        log::info!("App mounted, reporting GUI ready");
        trigger_gui_ready(Some(()));
    });

    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error listening to {}: {err}",
                common::events::health::APP_READY
            )
        }

        if let Some(err) = error_update_window_appearance() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::setup::UPDATE_WINDOW_APPEARANCE
            );
        }
        if let Some(err) = error_update_window_size() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::setup::UPDATE_WINDOW_SIZE
            );
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

    provide_layout_context();
    provide_playback_service();

    view! {
        <>
            <leptos_styling::StyleSheets />
            <div class=window_appearance_class>
                <main class="bg-red dark:bg-black font-serif text-black dark:text-red relative h-screen w-screen">
                    <Router>
                        <AppRoutes />
                    </Router>
                </main>
            </div>
        </>
    }
}
