use crate::components::{Button, Card, Icon};
use leptos::prelude::*;
use shared::{NavCommittedPayload, NavStartedPayload, RouteId};
use tauri_use::{use_invoke_with_args, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

#[component]
pub fn About() -> impl IntoView {
    // Listen for the commit event to get tx_id for enter_done (if available).
    let UseListenReturn {
        data: committed,
        error: error_committed,
        open: open_committed,
        ..
    } = use_listen::<NavCommittedPayload>(EventType::Custom(
        shared::events::navigation::NAV_COMMITTED,
    ));

    Effect::new(move |_| {
        open_committed();
    });

    Effect::new(move |_| {
        if let Some(err) = error_committed() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_COMMITTED
            )
        }
    });

    // Listen for navigation_started to handle leave (About -> Home).
    let UseListenReturn {
        data: started,
        error: error_started,
        open: open_started,
        ..
    } = use_listen::<NavStartedPayload>(EventType::Custom(shared::events::navigation::NAV_STARTED));

    Effect::new(move |_| {
        open_started();
    });

    Effect::new(move |_| {
        if let Some(err) = error_started() {
            log::error!(
                "Error listening to {}: {err}",
                shared::events::navigation::NAV_STARTED
            )
        }
    });

    let committed_tx_id = Signal::derive(move || {
        committed().as_ref().and_then(|p| {
            if p.to == RouteId::About {
                Some(p.tx_id)
            } else {
                None
            }
        })
    });
    let leave_tx_id = Signal::derive(move || {
        started().as_ref().and_then(|ev| {
            if ev.from == RouteId::About {
                Some(ev.tx_id)
            } else {
                None
            }
        })
    });

    // Unified animation signal driving Card
    let (start_animation, set_start_animation) =
        signal(None::<(u64, crate::components::CardAnimation)>);

    // Queue ENTER on commit to About
    Effect::new(move |_| {
        if let Some(tx) = committed_tx_id() {
            set_start_animation(Some((
                tx,
                crate::components::CardAnimation::EnterY {
                    from_deg: -90.0,
                    to_deg: 0.0,
                    ms: 600.0,
                },
            )));
            log::debug!("About: queued ENTER tx_id={}", tx);
        }
    });

    // Queue LEAVE on navigation start from About
    Effect::new(move |_| {
        if let Some(tx) = leave_tx_id() {
            set_start_animation(Some((
                tx,
                crate::components::CardAnimation::LeaveY {
                    from_deg: 0.0,
                    to_deg: 90.0,
                    ms: 600.0,
                },
            )));
            log::debug!("About: queued LEAVE tx_id={}", tx);
        }
    });

    // Trigger to notify backend that the enter animation has completed.
    let UseTauriWithReturn {
        trigger: enter_done_trigger,
        error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_ENTER_DONE,
    );

    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_ENTER_DONE,
                err
            );
        }
    });

    // Trigger navigation back to Home (used by Back button).
    let UseTauriWithReturn {
        trigger: navigate_trigger,
        error: navigate_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavigateRequestPayload, ()>(
        shared::commands::navigation::NAVIGATE,
    );

    Effect::new(move |_| {
        if let Some(err) = navigate_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAVIGATE,
                err
            );
        }
    });

    // Trigger to notify backend that the leave animation has completed (when navigating away from About).
    let UseTauriWithReturn {
        trigger: leave_done_trigger,
        error: leave_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_LEAVE_DONE,
    );

    Effect::new(move |_| {
        if let Some(err) = leave_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_LEAVE_DONE,
                err
            );
        }
    });

    view! {
        <div class="w-full h-full flex items-center justify-center">
            // Card receives tx signals and notifies backend when animations complete
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
                <div class="flex items-center justify-between gap-4 mb-4">
                    <Button
                        size="Md".to_string()
                        variant="Outline".to_string()
                        on:click=move |_| {
                            navigate_trigger(
                                Some(shared::commands::navigation::NavigateRequestPayload {
                                    path: "/".to_string(),
                                }),
                            );
                        }
                        class="relative pl-14"
                    >
                        <span class="absolute left-4 text-4xl">
                            <Icon name="back" stroke_width=12.0 />
                        </span>
                        Back
                    </Button>
                    <h1 class="block text-5xl text-center text-black dark:text-red">"About"</h1>
                </div>
                <p class="text-xl text-black/80 dark:text-red/90 max-w-[42ch] text-center">
                    "Red Siren is a playful instrument and tuner."
                </p>
            </Card>
        </div>
    }
}
