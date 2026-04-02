use common::RouteId;
use leptos::prelude::*;
use leptos_router::components::*;
use leptos_router::{hooks::use_location, location::Location, StaticSegment};
use leptos_use::{signal_debounced, use_window_size, UseWindowSizeReturn};
use tauri_use::{use_invoke, UseTauriReturn};

use crate::util::secondary_window::is_secondary_window;
use crate::{
    components::{AppError, ErrorTemplate, Intro},
    pages::{About, Donate, Edit, Home, Permissions, Play, Tune},
};

#[component]
pub fn AppRoutes() -> impl IntoView {
    // Provide a global in-app navigation stack (StoredValue) as context.
    let nav_stack: StoredValue<Vec<String>> = StoredValue::new(Vec::new());
    provide_context(nav_stack);

    let is_secondary_window = is_secondary_window();

    // Track path changes and push unique consecutive entries.
    let Location { pathname, .. } = use_location();
    Effect::new(move |_| {
        let p = pathname();
        nav_stack.update_value(|stack| {
            if stack.last().map(|last| last != &p).unwrap_or(true) {
                stack.push(p.clone());
            }
        });
    });

    let UseWindowSizeReturn { width, height } = use_window_size();

    let UseTauriReturn {
        trigger: trigger_update_window_size,
        error: error_update_window_size,
        ..
    } = use_invoke::<common::commands::setup::UpdateWindowSizePayload, (), ()>(
        common::commands::setup::UPDATE_WINDOW_SIZE,
    );

    let width = signal_debounced(width, 70.0);
    let height = signal_debounced(height, 70.0);

    Effect::new(move |_| {
        if let Some(err) = error_update_window_size() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::setup::UPDATE_WINDOW_SIZE
            );
        }
    });

    Effect::new(move |_| {
        let width = width();
        let height = height();

        if !is_secondary_window() {
            log::info!("Detected window resize: {width}, {height}");
            trigger_update_window_size(Some((
                common::commands::setup::UpdateWindowSizePayload { width, height },
                (),
            )));
        }
    });

    let container_class = move || {
        format!(
            "absolute pointer-events-auto z-1 {}",
            if is_secondary_window() {
                "min-h-full min-w-full overflow-auto"
            } else {
                "h-full w-full overflow-hidden"
            }
        )
    };

    view! {
        <Intro>
            <div class=container_class role="main">
                <Routes
                    fallback=|| {
                        let mut outside_errors = Errors::default();
                        outside_errors.insert_with_default_key(AppError::NotFound);
                        view! { <ErrorTemplate outside_errors /> }.into_view()
                    }
                    transition=true
                >
                    <Route path=(StaticSegment(RouteId::Home.as_ref()),) view=Home />
                    <Route path=(StaticSegment(RouteId::Play.as_ref()),) view=Play />
                    <Route path=(StaticSegment(RouteId::Tune.as_ref()),) view=Tune />
                    <Route path=(StaticSegment(RouteId::Edit.as_ref()),) view=Edit />
                    <Route path=(StaticSegment(RouteId::About.as_ref()),) view=About />
                    <Route path=(StaticSegment(RouteId::Donate.as_ref()),) view=Donate />
                    <Route path=(StaticSegment(RouteId::Permissions.as_ref()),) view=Permissions />
                </Routes>
            </div>
        </Intro>
    }
}
