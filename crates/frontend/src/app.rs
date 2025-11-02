use common::commands::setup::UpdateWindowAppearancePayload;
use leptos::prelude::*;
use leptos_router::components::*;
use leptos_use::use_preferred_dark;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::{
    components::{provide_tuner_service, Notifications},
    routes::AppRoutes,
    util::{
        layout_context::provide_layout_context,
        playback_service::provide_playback_service,
        setup_context::provide_setup_context,
        tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
    },
};

#[component]
pub fn App() -> impl IntoView {
    let UseTauriWithReturn {
        trigger: trigger_gui_ready,
        ..
    } = use_command::<()>(common::commands::health::GUI_READY);

    let UseTauriReturn {
        trigger: trigger_update_window_appearance,
        error: error_update_window_appearance,
        ..
    } = use_invoke::<UpdateWindowAppearancePayload, (), ()>(
        common::commands::setup::UPDATE_WINDOW_APPEARANCE,
    );

    let UseTauriResourceReturn {
        data: window_appearance_override,
        ..
    } = use_tauri_resource::<common::commands::setup::UpdateWindowAppearanceOverridePayload>(
        common::commands::setup::GET_WINDOW_APPEARANCE_OVERRIDE,
    );

    let preferred_dark = use_preferred_dark();

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
        log::info!("App mounted, reporting GUI ready");
        trigger_gui_ready(Some(()));
    });

    Effect::new(move |_| {
        if let Some(err) = error_update_window_appearance() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::setup::UPDATE_WINDOW_APPEARANCE
            );
        }
    });

    Effect::new(move |_| {
        let dark = preferred_dark();
        log::info!("Preferred dark mode: {dark}",);
        trigger_update_window_appearance(Some((UpdateWindowAppearancePayload { dark }, ())));
    });

    provide_layout_context();
    provide_playback_service();
    provide_tuner_service();
    provide_setup_context();

    view! {
        <>
            <leptos_styling::StyleSheets />
            <div class=window_appearance_class>
                <main class="bg-red dark:bg-black font-serif text-black dark:text-red relative h-screen w-screen select-none overflow-hidden">
                    <Router>
                        <AppRoutes />
                    </Router>
                    <Notifications />
                </main>
            </div>
        </>
    }
}
