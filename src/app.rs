use leptos::prelude::*;
use leptos_router::components::*;
use leptos_use::{use_preferred_dark, use_window_size, UseWindowSizeReturn};
use shared::commands::setup::UpdateWindowAppearancePayload;
use tauri_use::{
    use_command, use_invoke, use_listen, EventType, UseListenReturn, UseTauriReturn,
    UseTauriWithReturn,
};

use crate::{components::Intro, nav_commit_cache::NavCommitCache, routes};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveWindowContext(pub Option<(f64, f64)>);

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

    let UseWindowSizeReturn { width, height } = use_window_size();

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

    let (window, set_window) = signal(ActiveWindowContext(None));

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

    provide_context(window);
    provide_context(NavCommitCache::default());

    Effect::new(move |_| {
        if let Some(size) = event_id()
            .iter()
            .filter_map(|_| {
                let width = width();
                let height = height();
                if width > 0.0 && height > 0.0 {
                    Some((width, height))
                } else {
                    None
                }
            })
            .next()
        {
            set_window.set(ActiveWindowContext(Some(size)));
            log::info!(
                "App ready event received, active window: {:.0}x{:.0}",
                size.0,
                size.1
            );
            // Bootstrap initial navigation transaction (tx_id=0)
            bootstrap_trigger(Some(()));
        }
    });

    // Track and log initial and subsequent window size updates; update context on change
    Effect::new(move |_| {
        let w = width();
        let h = height();
        if w > 0.0 && h > 0.0 {
            let prev = window().0;
            let new = Some((w, h));
            if prev != new {
                set_window.set(ActiveWindowContext(new));
                if prev.is_some() {
                    log::info!("Window size updated: {:.0}x{:.0}", w, h);
                } else {
                    log::info!("Initial window size: {:.0}x{:.0}", w, h);
                }
            }
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
