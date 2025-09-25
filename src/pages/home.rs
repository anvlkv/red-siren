use leptos::prelude::*;
use shared::RouteId;
use tauri_use::{use_invoke_with_args, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

use crate::components::{Card, Menu};

#[component]
pub fn Home() -> impl IntoView {
    // Orchestrated enter/leave signals from backend events
    // Listen for committed (to == Home) -> enter
    let UseListenReturn {
        data: committed,
        error: error_committed,
        open: open_committed,
        ..
    } = use_listen::<shared::NavCommittedPayload>(EventType::Custom(
        shared::events::navigation::NAV_COMMITTED,
    ));
    Effect::new(move |_| open_committed());
    Effect::new(move |_| {
        if let Some(err) = error_committed() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_COMMITTED
            );
        }
    });

    // Listen for backend navigation_sync (UI reload alignment)
    let UseListenReturn {
        data: sync,
        error: error_sync,
        open: open_sync,
        ..
    } = use_listen::<shared::NavSyncPayload>(EventType::Custom(
        shared::events::navigation::NAV_SYNC,
    ));
    Effect::new(move |_| open_sync());
    Effect::new(move |_| {
        if let Some(err) = error_sync() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_SYNC
            );
        }
    });

    // Listen for started (from == Home) -> leave
    let UseListenReturn {
        data: started,
        error: error_started,
        open: open_started,
        ..
    } = use_listen::<shared::NavStartedPayload>(EventType::Custom(
        shared::events::navigation::NAV_STARTED,
    ));
    Effect::new(move |_| open_started());
    Effect::new(move |_| {
        if let Some(err) = error_started() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_STARTED
            );
        }
    });

    // Build enter/leave tx signals for the Card
    let start_enter_tx = Signal::derive(move || {
        committed().as_ref().and_then(|p| {
            if p.to == RouteId::Home {
                Some(p.tx_id)
            } else {
                None
            }
        })
    });
    let start_leave_tx = Signal::derive(move || {
        started().as_ref().and_then(|ev| {
            if ev.from == RouteId::Home {
                Some(ev.tx_id)
            } else {
                None
            }
        })
    });

    // One-time appear animation trigger (first time Home appears after sync/commit)
    let appear_tx = RwSignal::new(None::<u64>);
    let appear_started = RwSignal::new(false);
    Effect::new(move |_| {
        // If we haven't started appear animation yet, trigger when either:
        // - navigation_sync reports Home, or
        // - navigation_committed reports Home
        if !appear_started()
            && ((sync()
                .as_ref()
                .map(|p| p.to == RouteId::Home)
                .unwrap_or(false))
                || (committed()
                    .as_ref()
                    .map(|p| p.to == RouteId::Home)
                    .unwrap_or(false)))
        {
            appear_tx.set(Some(1));
            appear_started.set(true);
        }
    });

    // Triggers to notify backend that enter/leave animations completed
    let UseTauriWithReturn {
        trigger: enter_done_trigger,
        error: enter_done_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_ENTER_DONE,
    );
    Effect::new(move |_| {
        if let Some(err) = enter_done_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_ENTER_DONE,
                err
            );
        }
    });

    let UseTauriWithReturn {
        trigger: leave_done_trigger,
        error: leave_done_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_LEAVE_DONE,
    );
    Effect::new(move |_| {
        if let Some(err) = leave_done_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_LEAVE_DONE,
                err
            );
        }
    });

    view! {
        <div class="w-full h-full flex items-center justify-center">
            <Card
                start_appear_tx=Signal::derive(move || appear_tx())
                start_enter_tx=start_enter_tx
                start_leave_tx=start_leave_tx
                on_enter_done=Callback::new({
                    move |tx_id| {
                        enter_done_trigger(
                            Some(shared::commands::navigation::NavTxPayload {
                                tx_id,
                            }),
                        );
                    }
                })
                on_leave_done=Callback::new({
                    move |tx_id| {
                        leave_done_trigger(
                            Some(shared::commands::navigation::NavTxPayload {
                                tx_id,
                            }),
                        );
                    }
                })
            >
                <Menu />
            </Card>
        </div>
    }
}
