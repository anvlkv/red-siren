use leptos::prelude::*;
use shared::RouteId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::components::{Button, Card, CardAnimation, Icon, UiSize, UiVariant};

const BASE_ANIMATION_DURATION_MS: f64 = 600.0;

/// Configuration for one-time appear animation (used by Home page)
#[derive(Debug, Clone, Copy)]
pub struct AppearAnimationConfig {
    /// Base height for scaling calculations
    pub base_height: f64,
    /// Base Y translation distance
    pub base_y_px: f64,
    /// Base tilt angle
    pub tilt_x_from_deg: f32,
    /// Base duration in milliseconds
    pub base_ms: f64,
    /// Static flag to track if animation has been played
    pub played_flag: &'static OnceLock<AtomicBool>,
}

impl PartialEq for AppearAnimationConfig {
    fn eq(&self, other: &Self) -> bool {
        self.base_height == other.base_height
            && self.base_y_px == other.base_y_px
            && self.tilt_x_from_deg == other.tilt_x_from_deg
            && self.base_ms == other.base_ms
            && std::ptr::eq(self.played_flag, other.played_flag)
    }
}

impl Default for AppearAnimationConfig {
    fn default() -> Self {
        static DEFAULT_APPEAR_PLAYED: OnceLock<AtomicBool> = OnceLock::new();
        Self {
            base_height: 900.0,
            base_y_px: 800.0,
            tilt_x_from_deg: -60.0,
            base_ms: BASE_ANIMATION_DURATION_MS,
            played_flag: &DEFAULT_APPEAR_PLAYED,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationTx {
    Enter(u64),
    Leave(u64),
}

/// Generic Page component that handles common navigation animation logic.
///
/// Extracts the shared patterns from Home, About, and Donate pages:
/// - Event listening (NAV_COMMITTED, NAV_STARTED)
/// - Animation state management
/// - Tauri invoke setup for enter/leave done callbacks
/// - Error handling
/// - Card wrapper with animation coordination
///
/// Special features:
/// - Optional one-time appear animation (for Home page)
/// - Route-specific filtering
/// - Window-size scaling for appear animations
#[component]
pub fn ContentPage(
    /// The RouteId this page represents (for event filtering)
    route_id: RouteId,

    /// Page title
    #[prop(into, optional)]
    title: String,

    /// Page content
    children: Children,

    /// Optional appear animation config (for Home page first-time animation)
    #[prop(optional)]
    appear_animation_config: Option<AppearAnimationConfig>,

    #[prop(optional, into)] no_back_button: bool,
) -> impl IntoView {
    let nav_tx = expect_context::<Signal<Option<NavigationTx>>>();

    // Track whether a back navigation is currently possible.
    let (can_go_back, set_can_go_back) = signal(false);

    // Query: can we go back? (returns bool)
    let UseTauriWithReturn {
        trigger: can_go_back_trigger,
        data: can_go_back_data,
        error: can_go_back_error,
        ..
    } = use_invoke_with_args::<(), bool>(shared::commands::navigation::NAV_CAN_GO_BACK);

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
        if let Some(err) = can_go_back_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_CAN_GO_BACK,
                err
            );
        }
    });

    // Update local signal when query result arrives.
    Effect::new(move |_| {
        if let Some(v) = can_go_back_data() {
            set_can_go_back(v);
        }
    });

    // Initial query on mount.
    Effect::new(move |_| {
        can_go_back_trigger(Some(()));
    });

    // Re-query on entering a new route (after commit & enter animation begins).
    Effect::new(move |_| {
        if matches!(nav_tx(), Some(NavigationTx::Enter(_))) {
            can_go_back_trigger(Some(()));
        }
    });

    // Unified animation signal driving Card
    let (card_animation, set_card_animation) = signal({
        let initial = if let Some(config) = appear_animation_config.filter(|c| {
            let played = c.played_flag.get().unwrap();
            !played.load(Ordering::Relaxed)
        }) {
            Some(CardAnimation::Appear {
                x_from_px: 0.0,
                y_from_px: config.base_y_px as f32,
                tilt_x_from_deg: config.tilt_x_from_deg,
                ms: config.base_ms,
            })
        } else {
            Some(CardAnimation::EnterY {
                from_deg: 90.0,
                to_deg: 0.0,
                ms: BASE_ANIMATION_DURATION_MS,
            })
        };

        log::debug!("Initial animation: {initial:?}");

        initial
    });

    // Queue LEAVE animation on navigation start from this route
    Effect::new(move |_| {
        if matches!(nav_tx(), Some(NavigationTx::Leave(_))) {
            set_card_animation(Some(CardAnimation::LeaveY {
                from_deg: 0.0,
                to_deg: 90.0,
                ms: BASE_ANIMATION_DURATION_MS,
            }));
            log::debug!("Page({route_id:?}): queued LEAVE tx_id={:?}", nav_tx());
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

        if let Some(c) = card_animation()
            .filter(|a| matches!(a, CardAnimation::Appear { .. }))
            .and(appear_animation_config)
        {
            let played = c.played_flag.get().unwrap();
            played.store(true, Ordering::Relaxed);
        }

        set_card_animation(None);
    });

    view! {
        <div class="w-full h-full flex items-center justify-center">
            <Card
                start_animation=Signal::derive(card_animation)
                class="max-h-screen"
                on_animation_done=animation_done_cb
            >
                <div class="flex items-center justify-between gap-4 mb-6">
                    <Show when=move || !no_back_button && can_go_back()>
                        <Button
                            size=UiSize::Md
                            variant=UiVariant::Outline
                            on:click=move |_| {
                                back_trigger(Some(()));
                            }
                            class="relative pl-14"
                        >
                            <span class="absolute left-4 text-4xl">
                                <Icon name="back" />
                            </span>
                            Back
                        </Button>
                    </Show>
                    <h1 class="block text-5xl text-center italic">{title}</h1>
                </div>
                {children()}
            </Card>
        </div>
    }
}
