use crate::components::{Button, Card, Icon};
use leptos::prelude::*;
use leptos_use::use_preferred_dark;
use shared::{NavCommittedPayload, NavStartedPayload, RouteId};
use tauri_use::{use_invoke_with_args, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

#[component]
pub fn Donate() -> impl IntoView {
    let is_dark = use_preferred_dark();

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

    // Listen for navigation_started to handle leave (Donate -> About).
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
            if p.to == RouteId::Donate {
                Some(p.tx_id)
            } else {
                None
            }
        })
    });
    let leave_tx_id = Signal::derive(move || {
        started().as_ref().and_then(|ev| {
            if ev.from == RouteId::Donate {
                Some(ev.tx_id)
            } else {
                None
            }
        })
    });

    // Unified animation signal driving Card
    let (start_animation, set_start_animation) =
        signal(None::<(u64, crate::components::CardAnimation)>);

    // Queue ENTER on commit to Donate
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
            log::debug!("Donate: queued ENTER tx_id={}", tx);
        }
    });

    // Queue LEAVE on navigation start from Donate
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
            log::debug!("Donate: queued LEAVE tx_id={}", tx);
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

    // Trigger to notify backend that the leave animation has completed (when navigating away from Donate).
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
                                    route: RouteId::About,
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
                    <h1 class="block text-5xl text-center italic">"Donate"</h1>
                </div>
                <div class="flex flex-col items-center justify-center gap-6">
                    <h2 class="text-2xl italic">"Support the Creator of Red Siren"</h2>
                    <p class="text-xl max-w-[42ch] ">
                        "Red Siren is a labor of love — designed, built, and shared with a passion for sound and creativity."
                    </p>
                    <p class="text-xl max-w-[42ch] ">
                        "If Red Siren has sparked something in you, consider donating. Every bit helps, and I’m deeply grateful for your kindness. Thank you for believing in this work."
                    </p>

                    <div class="contents">
                        {move || {
                            if is_dark() {
                                view! {
                                    <a
                                        href="https://nowpayments.io/donation?api_key=5014fba8-64de-4526-84c1-527cd621d274"
                                        target="_blank"
                                        rel="noreferrer noopener"
                                    >
                                        <img
                                            src="https://nowpayments.io/images/embeds/donation-button-black.svg"
                                            alt="Crypto donation button by NOWPayments"
                                        />
                                    </a>
                                }
                            } else {
                                view! {
                                    <a
                                        href="https://nowpayments.io/donation?api_key=5014fba8-64de-4526-84c1-527cd621d274"
                                        target="_blank"
                                        rel="noreferrer noopener"
                                    >
                                        <img
                                            src="https://nowpayments.io/images/embeds/donation-button-white.svg"
                                            alt="Cryptocurrency & Bitcoin donation button by NOWPayments"
                                        />
                                    </a>
                                }
                            }
                        }}
                    </div>
                    <p class="text-xl max-w-[42ch]">
                        "Your support helps me keep going. It fuels the time, care, and resources needed to craft each instrument and bring this project to life."
                    </p>
                </div>
            </Card>
        </div>
    }
}
