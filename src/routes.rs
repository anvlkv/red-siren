use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::{
    components::*,
    hooks::{use_location, use_navigate},
    path,
};
use tauri_use::{use_invoke_with_args, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

use crate::components::{AppError, ErrorTemplate};
use crate::pages::{About, Home};

#[component]
pub fn AppRoutes() -> impl IntoView {
    // Listen for committed navigation events; perform actual client-side route change here.
    let UseListenReturn {
        data: committed,
        error,
        open,
        ..
    } = use_listen::<shared::NavCommittedPayload>(EventType::Custom(
        shared::events::navigation::NAV_COMMITTED,
    ));
    let navigate = use_navigate();
    let location = use_location();
    let UseTauriWithReturn {
        error: invoke_nav_syn_error,
        trigger: trigger_nav_sync,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavSyncRequestPayload, ()>(
        shared::commands::navigation::NAV_SYNC,
    );

    // Listen for navigation_sync (UI reload alignment)
    let UseListenReturn {
        data: sync,
        error: sync_error,
        open: sync_open,
        ..
    } = use_listen::<shared::NavSyncPayload>(EventType::Custom(
        shared::events::navigation::NAV_SYNC,
    ));
    let navigate_from_sync = navigate.clone();

    Effect::new(move |_| {
        open();
        sync_open();
        let current = location.pathname.get();
        trigger_nav_sync(Some(shared::commands::navigation::NavSyncRequestPayload {
            path: current,
        }));
    });

    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_COMMITTED
            );
        }
        if let Some(err) = sync_error() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_SYNC
            );
        }
        if let Some(err) = invoke_nav_syn_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::navigation::NAV_SYNC
            );
        }
    });

    Effect::new(move |_| {
        if let Some(payload) = committed().as_ref() {
            navigate(&payload.path, NavigateOptions::default());
        }
    });

    Effect::new(move |_| {
        if let Some(payload) = sync().as_ref() {
            navigate_from_sync(&payload.path, NavigateOptions::default());
        }
    });

    view! {
        <Routes fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors /> }.into_view()
        }>
            <Route path=path!("/") view=move || view! { <Home /> } />
            <Route path=path!("/about") view=move || view! { <About /> } />
            <Route path=path!("/play") view=move || view! { <div>"Play"</div> } />
            <Route path=path!("/permissions") view=move || view! { <div>"Permissions"</div> } />

        </Routes>
    }
}
