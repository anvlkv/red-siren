use std::str::FromStr;

use leptos::prelude::*;
use leptos_router::{
    components::*,
    hooks::{use_location, use_navigate},
};
use leptos_router::{NavigateOptions, StaticSegment};
use shared::RouteId;
use tauri_use::{use_invoke_with_args, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

use crate::components::{AppError, ErrorTemplate};
use crate::pages::{About, Donate, Home};

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
            route: RouteId::from_str(&current).unwrap_or(RouteId::Home),
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
            navigate(payload.to.into(), NavigateOptions::default());
        }
    });

    Effect::new(move |_| {
        if let Some(payload) = sync().as_ref() {
            navigate_from_sync(payload.to.into(), NavigateOptions::default());
        }
    });

    view! {
        <Routes fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors /> }.into_view()
        }>
            <Route path=(StaticSegment(RouteId::Home.as_ref()),) view=move || view! { <Home /> } />
            <Route
                path=(StaticSegment(RouteId::About.as_ref()),)
                view=move || view! { <About /> }
            />
            <Route
                path=(StaticSegment(RouteId::Donate.as_ref()),)
                view=move || view! { <Donate /> }
            />
            <Route
                path=(StaticSegment(RouteId::Play.as_ref()),)
                view=move || view! { <div>"Play"</div> }
            />
            <Route
                path=(StaticSegment(RouteId::Tune.as_ref()),)
                view=move || view! { <div>"Tune"</div> }
            />
            <Route
                path=(StaticSegment(RouteId::Permissions.as_ref()))
                view=move || view! { <div>"Permissions"</div> }
            />

        </Routes>
    }
}
