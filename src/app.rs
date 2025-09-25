use leptos::prelude::*;
use leptos_router::components::*;
use leptos_use::{use_window_size, UseWindowSizeReturn};
use tauri_use::{use_command, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

use crate::{components::Intro, routes};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveWindowContext(pub Option<(f64, f64)>);

#[component]
pub fn App() -> impl IntoView {
    let UseTauriWithReturn { trigger, .. } = use_command::<()>(shared::commands::health::GUI_READY);
    let UseTauriWithReturn {
        trigger: sync_trigger,
        ..
    } = use_command::<()>("navigation_sync");

    let UseListenReturn {
        event_id,
        open,
        error,
        ..
    } = use_listen::<()>(EventType::Custom(shared::events::health::APP_READY));

    Effect::new(move |_| {
        open();

        log::info!("App mounted, reporting GUI ready");
        trigger(Some(()));
    });

    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::health::APP_READY
            )
        }
    });

    let UseWindowSizeReturn { width, height } = use_window_size();

    let (window, set_window) = signal(ActiveWindowContext(None));

    provide_context(window);

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
            log::info!("App ready event received, active window context set");
            // Request backend navigation snapshot to sync router after reloads
            sync_trigger(Some(()));
        }
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
