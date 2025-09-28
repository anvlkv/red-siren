use leptos::prelude::*;
use shared::commands::navigation::NavigateRequestPayload;
use shared::RouteId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::components::{Button, Card, CardAnimation, Icon, UiSize, UiVariant};

const BASE_ANIMATION_DURATION_MS: f64 = 600.0;

/// Configuration for one-time appear animation (used by Home page)
#[derive(Debug, Clone)]
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

    /// The RouteId to go back to when leaving the page
    #[prop(into, optional)]
    route_back: Option<RouteId>,

    /// Page title
    #[prop(into, optional)]
    title: String,

    /// Page content
    children: Children,

    /// Optional appear animation config (for Home page first-time animation)
    #[prop(optional)]
    appear_animation_config: Option<AppearAnimationConfig>,
) -> impl IntoView {
    let nav_tx = expect_context::<Signal<Option<NavigationTx>>>();

    let UseTauriWithReturn {
        trigger: navigate_trigger,
        error: navigate_error,
        ..
    } = use_invoke_with_args::<NavigateRequestPayload, ()>(shared::commands::navigation::NAVIGATE);

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
        if let Some(err) = navigate_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAVIGATE,
                err
            );
        }
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
    });

    // Unified animation signal driving Card
    let (card_animation, set_card_animation) = signal({
        if let Some((config, played)) = appear_animation_config
            .iter()
            .filter_map(|c| {
                let played = c.played_flag.get_or_init(|| AtomicBool::new(false));
                if !played.load(Ordering::Relaxed) {
                    Some((c, played))
                } else {
                    None
                }
            })
            .next()
        {
            played.store(true, Ordering::Relaxed);
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
        }
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
                    <Show when=move || route_back.is_some()>
                        <Button
                            size=UiSize::Md
                            variant=UiVariant::Outline
                            on:click=move |_| {
                                navigate_trigger(
                                    route_back.map(|route| NavigateRequestPayload { route }),
                                );
                            }
                            class="relative pl-14"
                        >
                            <span class="absolute left-4 text-4xl">
                                <Icon name="back" stroke_width=12.0 />
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
