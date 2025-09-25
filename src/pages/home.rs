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

    // Unified animation signal (single source driving Card)
    let (start_animation, set_start_animation) =
        signal(None::<(u64, crate::components::CardAnimation)>);

    // Access window size context to scale appear animation for screen size
    let window = use_context::<Signal<crate::app::ActiveWindowContext>>();

    // One-time appear animation trigger (first time Home appears after commit), scaled by window size
    let appear_started = RwSignal::new(false);
    Effect::new(move |_| {
        if appear_started() {
            return;
        }

        // Wait for a valid window height before triggering appear
        let size = window.as_ref().and_then(|w| w().0);
        if let Some((_, height)) = size {
            // Trigger when navigation was committed to Home
            if committed()
                .as_ref()
                .map(|p| p.to == RouteId::Home)
                .unwrap_or(false)
            {
                // Scale translation and duration by height (clamped)
                let base_h = 900.0;
                let mut y_scale = height / base_h;
                y_scale = y_scale.clamp(0.75, 1.25);
                let y_from_px = (800.0 * y_scale) as f32;

                let mut t_scale = height / base_h;
                t_scale = t_scale.clamp(0.85, 1.15);
                let ms = 800.0 * t_scale;

                set_start_animation(Some((
                    1,
                    crate::components::CardAnimation::Appear {
                        x_from_px: 0.0,
                        y_from_px,
                        tilt_x_from_deg: -60.0,
                        ms,
                    },
                )));
                appear_started.set(true);
                log::info!(
                    "Home: queued APPEAR (y_from_px={:.1}, ms={:.0}) for height {:.0}",
                    y_from_px,
                    ms,
                    height
                );
            }
        }
    });

    // Enter when navigation is committed to Home
    Effect::new(move |_| {
        if let Some(tx) = start_enter_tx() {
            set_start_animation(Some((
                tx,
                crate::components::CardAnimation::EnterY {
                    from_deg: -90.0,
                    to_deg: 0.0,
                    ms: 600.0,
                },
            )));
            log::debug!("Home: queued ENTER tx_id={}", tx);
        }
    });

    // Leave when navigation starts from Home
    Effect::new(move |_| {
        if let Some(tx) = start_leave_tx() {
            set_start_animation(Some((
                tx,
                crate::components::CardAnimation::LeaveY {
                    from_deg: 0.0,
                    to_deg: 90.0,
                    ms: 600.0,
                },
            )));
            log::debug!("Home: queued LEAVE tx_id={}", tx);
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
                start_animation=Signal::derive(start_animation)
                on_animation_done=Callback::new({
                    move |(tx_id, kind)| {
                        match kind {
                            crate::components::CardAnimation::EnterY { .. } => {
                                enter_done_trigger(
                                    Some(shared::commands::navigation::NavTxPayload {
                                        tx_id,
                                    }),
                                );
                            }
                            crate::components::CardAnimation::LeaveY { .. } => {
                                leave_done_trigger(
                                    Some(shared::commands::navigation::NavTxPayload {
                                        tx_id,
                                    }),
                                );
                            }
                            crate::components::CardAnimation::Appear { .. } => {}
                        }
                    }
                })
            >
                <Menu />
            </Card>
        </div>
    }
}
