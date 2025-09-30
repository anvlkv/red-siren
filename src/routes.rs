use std::str::FromStr;

use leptos::prelude::*;
use leptos_router::{
    components::*,
    hooks::{use_location, use_navigate},
};
use leptos_router::{NavigateOptions, StaticSegment};
use shared::{NavCommittedPayload, NavStartedPayload, RouteId};
use tauri_use::{use_invoke_with_args, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

use crate::{
    components::{AppError, ErrorTemplate},
    pages::Permissions,
};
use crate::{
    components::{Intro, NavigationTx},
    pages::{About, Donate, Home, Play, Tune},
};

#[component]
pub fn AppRoutes() -> impl IntoView {
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
        close: close_sync,
        ..
    } = use_listen::<shared::NavSyncPayload>(EventType::Custom(
        shared::events::navigation::NAV_SYNC,
    ));
    // Listen for the commit event to get tx_id for enter_done
    let UseListenReturn {
        data: committed,
        error: error_committed,
        open: open_committed,
        close: close_committed,
        ..
    } = use_listen::<NavCommittedPayload>(EventType::Custom(
        shared::events::navigation::NAV_COMMITTED,
    ));

    // Listen for navigation_started to handle leave animations
    let UseListenReturn {
        data: started,
        error: error_started,
        open: open_started,
        close: close_started,
        ..
    } = use_listen::<NavStartedPayload>(EventType::Custom(shared::events::navigation::NAV_STARTED));

    let nav_tx = Signal::derive(move || {
        let started = started();
        let committed = committed();
        log::debug!("Navigation. Started: {started:?}. Commited: {committed:?}");

        match (started, committed) {
            (None, None) => None,
            (None, Some(tx)) => Some(NavigationTx::Enter(tx.tx_id)),
            (Some(tx), None) => Some(NavigationTx::Leave(tx.tx_id)),
            (Some(tx_started), Some(tx_commited)) => {
                if tx_started.tx_id > tx_commited.tx_id {
                    Some(NavigationTx::Leave(tx_started.tx_id))
                } else {
                    Some(NavigationTx::Enter(tx_commited.tx_id))
                }
            }
        }
    });

    provide_context(nav_tx);

    Effect::new(move |_| {
        sync_open();
        open_started();
        open_committed();
        // this effect supposed to only run once, therefore we get pathname untracked.
        let current = location.pathname.get_untracked();
        trigger_nav_sync(Some(shared::commands::navigation::NavSyncRequestPayload {
            route: RouteId::from_str(&current).unwrap_or(RouteId::Home),
        }));
    });

    Effect::new(move |_| {
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
        if let Some(err) = error_committed() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_COMMITTED
            );
        }
        if let Some(err) = error_started() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_STARTED
            );
        }
    });

    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            if let Some(payload) = committed().as_ref() {
                navigate(payload.to.into(), NavigateOptions::default());
            }
        }
    });

    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            if let Some(payload) = sync().as_ref() {
                navigate(payload.to.into(), NavigateOptions::default());
            }
        }
    });

    on_cleanup(move || {
        close_sync();
        close_committed();
        close_started();
    });

    view! {
        <>
            <div
                class="absolute h-full w-full overflow-hidden pointer-events-none z-0"
                role="presentation"
            >
                <Intro />
            </div>
            <div class="absolute h-full w-full overflow-hidden pointer-events-auto z-1" role="main">
                <Routes fallback=|| {
                    let mut outside_errors = Errors::default();
                    outside_errors.insert_with_default_key(AppError::NotFound);
                    view! { <ErrorTemplate outside_errors /> }.into_view()
                }>
                    <Route path=(StaticSegment(RouteId::None.as_ref()),) view=|| view! { <></> } />
                    <Route path=(StaticSegment(RouteId::Home.as_ref()),) view=Home />
                    <Route path=(StaticSegment(RouteId::Play.as_ref()),) view=Play />
                    <Route path=(StaticSegment(RouteId::Tune.as_ref()),) view=Tune />
                    <Route path=(StaticSegment(RouteId::About.as_ref()),) view=About />
                    <Route path=(StaticSegment(RouteId::Donate.as_ref()),) view=Donate />
                    <Route path=(StaticSegment(RouteId::Permissions.as_ref()),) view=Permissions />
                </Routes>
            </div>
        </>
    }
}
