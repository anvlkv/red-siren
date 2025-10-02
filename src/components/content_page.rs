use leptos::prelude::*;
use leptos_use::use_window_size;
use shared::RouteId;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::components::{Button, Card, CardAnimation, Icon, UiSize, UiVariant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationTx {
    Enter(u64),
    Leave(u64),
}

/// Generic Page component that handles common navigation animation logic.
#[component]
pub fn ContentPage(
    /// The RouteId this page represents (for event filtering)
    route_id: RouteId,

    /// Page title
    #[prop(into, optional)]
    title: String,

    /// Page content
    children: Children,

    #[prop(optional, into)] no_back_button: bool,
) -> impl IntoView {
    let nav_tx = expect_context::<Signal<Option<NavigationTx>>>();

    // Back navigation (history) command (no payload)
    let UseTauriWithReturn {
        trigger: back_trigger,
        error: back_error,
        ..
    } = use_invoke_with_args::<(), ()>(shared::commands::navigation::NAV_BACK);

    // Trigger to notify backend that the enter animation has completed
    let UseTauriWithReturn {
        trigger: enter_done_trigger,
        error: enter_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_ENTER_DONE,
    );

    // Trigger to notify backend that the leave animation has completed
    let UseTauriWithReturn {
        trigger: leave_done_trigger,
        error: leave_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_LEAVE_DONE,
    );

    Effect::new(move |_| {
        if let Some(err) = enter_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_ENTER_DONE,
                err
            );
        }
        if let Some(err) = leave_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_LEAVE_DONE,
                err
            );
        }
        if let Some(err) = back_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_BACK,
                err
            );
        }
    });

    // Unified animation signal driving Card.
    // Enter / leave travel distances derived dynamically from current window size (no hardcoded px).
    let window_size = use_window_size();

    // Helper closures so we recalc on demand (resize, nav events).
    let compute_enter = move || {
        let w = window_size.width.get() as f32;
        let offset_x = -(w / 2.0 + 100.0); // Start just off-screen based on half width
        let depth_z = -w * 0.45; // Depth proportional to width for consistent perspective
        let rot = (w / 1600.0).min(1.0) * 55.0; // Scale rotation up to 55deg at large widths
        CardAnimation::EnterTravel3D {
            from_x_px: offset_x,
            from_z_px: depth_z,
            from_rot_y_deg: rot,
            to_rot_y_deg: 0.0,
        }
    };

    let compute_leave = move || {
        let w = window_size.width.get() as f32;
        let offset_x = w / 2.0 + 100.0; // Exit off opposite side
        let depth_z = -w * 0.45;
        let rot = -(w / 1600.0).min(1.0) * 55.0;
        CardAnimation::LeaveTravel3D {
            to_x_px: offset_x,
            to_z_px: depth_z,
            to_rot_y_deg: rot,
        }
    };

    // Signal to track when animation should be cleared after completion
    let (enter_animation_done, set_enter_animation_done) = signal(false);

    // Compute animation based on navigation transaction state
    let card_animation = Memo::new(move |_| {
        // Read all reactive values first
        let enter_animation_done = enter_animation_done();
        let nav_tx = nav_tx();
        let leave = compute_leave();
        let enter = compute_enter();

        if let Some(NavigationTx::Leave(_)) = nav_tx {
            log::debug!(
                "Page({:?}): queued LEAVE_TRAVEL (dynamic) tx_id={:?}",
                route_id,
                nav_tx
            );
            Some(leave)
        } else if enter_animation_done {
            None
        } else {
            Some(enter)
        }
    });

    let animation_done_cb = Callback::new(move |_| {
        match nav_tx() {
            Some(NavigationTx::Enter(tx_id)) => {
                enter_done_trigger(Some(shared::commands::navigation::NavTxPayload { tx_id }));
            }
            Some(NavigationTx::Leave(tx_id)) => {
                leave_done_trigger(Some(shared::commands::navigation::NavTxPayload { tx_id }));
            }
            None => {}
        }

        // Clear animation so Card can settle
        set_enter_animation_done(true);
    });

    view! {
        <div class="w-full h-full flex items-center justify-center">
            <Card
                start_animation=Signal::derive(card_animation)
                class="max-h-screen"
                on_animation_done=animation_done_cb
            >
                <div class="flex items-center justify-between gap-4 mb-6">
                    <Show when=move || !no_back_button>
                        <Button
                            size=UiSize::Md
                            variant=UiVariant::Outline
                            on:click=move |_| {
                                back_trigger(Some(()));
                            }
                        >
                            <Icon name="back" size=UiSize::Md />
                            <span class="inline-block ml-2 flex-grow text-center">Back</span>
                        </Button>
                    </Show>
                    <h1 class="block text-5xl text-center italic">{title}</h1>
                </div>
                {children()}
            </Card>
        </div>
    }
}
