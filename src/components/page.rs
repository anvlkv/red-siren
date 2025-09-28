use leptos::prelude::*;
use leptos_use::{use_window_size, UseWindowSizeReturn};
use shared::commands::navigation::NavigateRequestPayload;
use shared::{NavCommittedPayload, NavStartedPayload, RouteId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri_use::{use_invoke_with_args, use_listen, EventType, UseListenReturn, UseTauriWithReturn};

use crate::components::{Button, Card, CardAnimation, Icon};

const BASE_ANIMATION_DURATION_MS: f64 = 600.0;

/// Configuration for one-time appear animation (used by Home page)
#[derive(Debug, Clone)]
pub struct AppearAnimationConfig {
    /// Base height for scaling calculations
    pub base_height: f32,
    /// Base Y translation distance
    pub base_y_px: f32,
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
pub fn Page(
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
    let UseTauriWithReturn {
        trigger: navigate_trigger,
        error: navigate_error,
        ..
    } = use_invoke_with_args::<NavigateRequestPayload, ()>(shared::commands::navigation::NAVIGATE);

    Effect::new(move |_| {
        if let Some(err) = navigate_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAVIGATE,
                err
            );
        }
    });

    // Listen for the commit event to get tx_id for enter_done
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
            );
        }
    });

    // Listen for navigation_started to handle leave animations
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
            );
        }
    });

    // Derive tx_id signals for this specific route
    let committed_tx_id = Signal::derive(move || {
        committed().as_ref().and_then(|p| {
            if p.to == route_id {
                Some(p.tx_id)
            } else {
                None
            }
        })
    });

    let leave_tx_id = Signal::derive(move || {
        started().as_ref().and_then(|ev| {
            if ev.from == route_id {
                Some(ev.tx_id)
            } else {
                None
            }
        })
    });

    // Determine if navigation is backward using route_back prop
    let is_backward_navigation = Signal::derive(move || {
        started()
            .as_ref()
            .map(|ev| {
                // If navigating to the route_back, it's backward navigation
                route_back == Some(ev.to)
            })
            .unwrap_or(false)
    });

    // Unified animation signal driving Card
    let (start_animation, set_start_animation) = signal(None::<(u64, CardAnimation)>);

    // Handle optional appear animation (Home page special case)
    if let Some(config) = appear_animation_config {
        let UseWindowSizeReturn { width: _, height } = use_window_size();
        let appear_started = RwSignal::new(false);
        let played = config.played_flag.get_or_init(|| AtomicBool::new(false));

        Effect::new(move |_| {
            if played.load(Ordering::Relaxed) || appear_started() {
                return;
            }

            // Wait for a valid window height before triggering appear (cold mount only)
            let h = height() as f32;
            if h > 0.0 && committed_tx_id().is_none() {
                // Scale translation and duration by height (clamped)
                let y_scale = (h / config.base_height).clamp(0.75, 1.25);
                let y_from_px = config.base_y_px * y_scale;

                let t_scale = (h / config.base_height).clamp(0.85, 1.15);
                let ms = config.base_ms * t_scale as f64;

                set_start_animation(Some((
                    0,
                    CardAnimation::Appear {
                        x_from_px: 0.0,
                        y_from_px,
                        tilt_x_from_deg: config.tilt_x_from_deg,
                        ms,
                    },
                )));
                appear_started.set(true);
                played.store(true, Ordering::Relaxed);
                log::info!(
                    "Page({:?}): queued APPEAR (y_from_px={:.1}, ms={:.0}) for height {:.0}",
                    route_id,
                    y_from_px,
                    ms,
                    h
                );
            }
        });
    }

    // Queue ENTER animation on commit to this route
    Effect::new(move |_| {
        if let Some(tx) = committed_tx_id() {
            // Continuous rotation: backward enters from right, forward enters from left
            let from_deg = if is_backward_navigation() {
                90.0
            } else {
                -90.0
            };
            set_start_animation(Some((
                tx,
                CardAnimation::EnterY {
                    from_deg,
                    to_deg: 0.0,
                    ms: BASE_ANIMATION_DURATION_MS,
                },
            )));
            log::debug!(
                "Page({:?}): queued ENTER tx_id={} (backward={})",
                route_id,
                tx,
                is_backward_navigation()
            );
        }
    });

    // Queue LEAVE animation on navigation start from this route
    Effect::new(move |_| {
        if let Some(tx) = leave_tx_id() {
            // Continuous rotation: backward leaves left, forward leaves right
            let to_deg = if is_backward_navigation() {
                -90.0
            } else {
                90.0
            };
            set_start_animation(Some((
                tx,
                CardAnimation::LeaveY {
                    from_deg: 0.0,
                    to_deg,
                    ms: BASE_ANIMATION_DURATION_MS,
                },
            )));
            log::debug!(
                "Page({:?}): queued LEAVE tx_id={} (backward={})",
                route_id,
                tx,
                is_backward_navigation()
            );
        }
    });

    // Trigger to notify backend that the enter animation has completed
    let UseTauriWithReturn {
        trigger: enter_done_trigger,
        error: enter_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_ENTER_DONE,
    );

    Effect::new(move |_| {
        if let Some(err) = enter_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_ENTER_DONE,
                err
            );
        }
    });

    // Trigger to notify backend that the leave animation has completed
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
            <Card
                start_animation=Signal::derive(start_animation)
                on_animation_done=Callback::new({
                    move |(tx_id, kind)| {
                        match kind {
                            CardAnimation::EnterY { .. } => {
                                enter_done_trigger(
                                    Some(shared::commands::navigation::NavTxPayload {
                                        tx_id,
                                    }),
                                );
                            }
                            CardAnimation::LeaveY { .. } => {
                                leave_done_trigger(
                                    Some(shared::commands::navigation::NavTxPayload {
                                        tx_id,
                                    }),
                                );
                            }
                            CardAnimation::Appear { .. } => {}
                        }
                    }
                })
            >
                <div class="flex items-center justify-between gap-4 mb-6">
                    <Show when=move || route_back.is_some()>
                        <Button
                            size="Md".to_string()
                            variant="Outline".to_string()
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
