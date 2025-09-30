mod animation;
mod static_path;
mod suns;
mod wavering;

use keyframe::{keyframes, AnimationSequence};
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use leptos_use::{
    use_prefers_reduced_motion, use_raf_fn_with_options, utils::Pausable, UseRafFnCallbackArgs,
    UseRafFnOptions,
};
use static_path::*;
use suns::*;
use wavering::*;

use crate::util::{
    animation::ReducedMotionState,
    tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};
use shared::{NavStartedPayload, RouteId};
use std::str::FromStr;

pub use animation::*;

/// Refactored Intro:
/// - Removed external props (`animation`, `on_animation_ended`)
/// - Introduces internal animation pair signal (future: driven by navigation context)
/// - Tuner path intentionally skipped (todo!())
#[component]
pub fn Intro() -> impl IntoView {
    // ========== User Preferences ==========

    let reduced_motion = use_prefers_reduced_motion();

    // ========== Animation State Management ==========

    // Animation driving signal (tracks current transition: None | Some(from, to))
    let (animation_pair, set_animation_pair) =
        signal::<Option<(IntroAnimationTarget, IntroAnimationTarget)>>(None);

    // Animation timeline state
    let animation_state = RwSignal::new({
        let state = IntroAnimationState::default();
        keyframes![(state, 0.0)]
    });

    // Reduced motion tracking
    let reduced_motion_state = RwSignal::new(ReducedMotionState {
        total_keyframes: animation_state.get_untracked().keyframes(),
        current_keyframe: 0,
        accumulated_time: 0.0,
    });

    // Animation completion flag
    let completed = RwSignal::new(false);

    // SVG visibility state (hidden when showing instrument/tuner)
    let hidden_svgs = RwSignal::new(false);

    // ========== External Context & Resources ==========

    // Navigation context
    let nav_tx = expect_context::<Signal<Option<crate::components::NavigationTx>>>();
    let nav_started = expect_context::<Signal<Option<NavStartedPayload>>>();

    // Current location for initial state
    let location = use_location();

    // Instrument layout resource (fetched via invoke/event)
    let UseTauriResourceReturn {
        data: instrument_layout,
        ..
    } = use_tauri_resource::<shared::instrument::Layout>(shared::instrument::events::LAYOUT);

    // ========== Helper Functions ==========

    fn is_content(r: RouteId) -> bool {
        matches!(
            r,
            RouteId::Home | RouteId::About | RouteId::Donate | RouteId::Permissions
        )
    }

    // ========== Initial State Setup ==========

    // Initialize correct animation state based on current route
    // Track instrument_layout so we update when it arrives
    Effect::new(move |_| {
        let current_path = location.pathname.get_untracked();
        let current_route = RouteId::from_str(&current_path).unwrap_or(RouteId::Home);
        let layout = instrument_layout(); // Track this signal

        log::debug!(
            "Intro state check: route = {:?}, layout available = {}",
            current_route,
            layout.is_some()
        );

        // Only initialize if we haven't already animated to this state
        if current_route == RouteId::Play {
            if let Some(layout) = layout {
                // Check if we're already in the correct state
                if animation_pair.get_untracked().is_none() && !completed.get_untracked() {
                    log::debug!("Initializing Intro in Instrument state for Play route");
                    let target_state =
                        IntroAnimationState::from(IntroAnimationTarget::Instrument(layout));
                    animation_state.set(keyframes![(target_state, 0.0)]);
                    hidden_svgs.set(true);
                    completed.set(true);
                }
            } else {
                log::debug!("On Play route but instrument layout not yet available");
            }
        } else if current_route == RouteId::Tune {
            // TODO: Handle Tune route once backend is ready
            log::debug!("Tune route initialization not yet implemented");
        } else if is_content(current_route) {
            // Only set if not already in correct state
            if animation_pair.get_untracked().is_none() && !completed.get_untracked() {
                hidden_svgs.set(false);
                completed.set(true);
            }
        }
    });

    // ========== Animation Loop (RAF) ==========

    let Pausable { pause, resume, .. } = use_raf_fn_with_options(
        move |UseRafFnCallbackArgs { delta, .. }| {
            let mut state = animation_state.get_untracked();
            // Variables to track final state for completion check
            let mut final_time = state.time();
            let mut final_duration = state.duration();

            // Skip processing if delta is 0 (first frame or paused)
            if delta == 0.0 {
                return;
            }

            let _remainder = if reduced_motion.get_untracked() {
                let mut rm_state = reduced_motion_state.get_untracked();
                rm_state.accumulated_time += delta;

                // Get the current keyframe pair (current, next)
                let (_, next_kf) = state.pair();
                if let Some(next_kf) = next_kf {
                    // If we've accumulated enough time to reach the next keyframe
                    if rm_state.accumulated_time >= next_kf.time() {
                        // Jump directly to the next keyframe time
                        let remainder = state.advance_to(next_kf.time());
                        rm_state.current_keyframe += 1;
                        // Store values before moving state
                        final_time = state.time();
                        final_duration = state.duration();
                        animation_state.set(state);
                        reduced_motion_state.set(rm_state);
                        log::debug!(
                            "Advanced to keyframe; current_keyframe = {}, remainder = {:?}",
                            rm_state.current_keyframe,
                            remainder
                        );
                        Some(remainder)
                    } else {
                        // Not enough time accumulated yet, just update state
                        reduced_motion_state.set(rm_state);
                        None
                    }
                } else {
                    // No next keyframe (animation finished)
                    reduced_motion_state.set(rm_state);
                    log::debug!("No next keyframe; reduced motion animation finished");
                    None
                }
            } else {
                let rem = state.duration() - state.time();
                let remainder = state.advance_by(delta.min(rem));
                // Store values before moving state
                final_time = state.time();
                final_duration = state.duration();
                animation_state.set(state);
                Some(remainder)
            };

            // Mark completed only if animation truly finished (reached duration with actual progress)
            if final_time >= final_duration && final_duration > 0.0 && delta > 0.0 {
                completed.set(true);
                log::debug!(
                    "Animation marked completed (time {} >= duration {})",
                    final_time,
                    final_duration
                );
            }
        },
        UseRafFnOptions::default().immediate(false),
    );

    // ========== Navigation Transition Logic ==========

    // Combine navigation state into a derived signal
    let nav_transition = Signal::derive(move || {
        let started = nav_started();
        let tx = nav_tx();
        match (started, tx) {
            (Some(s), Some(t)) => Some((s, t)),
            _ => None,
        }
    });

    // Effect: Trigger animations based on navigation transitions
    Effect::new(move |_| {
        // Only track the memo, not individual signals
        let transition_data = nav_transition();

        // Cache the instrument layout to avoid multiple reactive accesses
        let cached_layout = instrument_layout.get_untracked();

        let _interrupted = animation_pair.get_untracked().is_some();
        if _interrupted {
            log::debug!(
                "Animation was interrupted; current pair = {:?}",
                animation_pair.get_untracked()
            );
        }

        if let Some((started, nav_state)) = transition_data {
            log::debug!("Navigation started payload = {:?}", started);
            match nav_state {
                crate::components::NavigationTx::Leave(_)
                    if is_content(started.from) && started.to == RouteId::Play =>
                {
                    log::debug!(
                        "Detected Leave transition from content -> Play (from = {:?}, to = {:?})",
                        started.from,
                        started.to
                    );
                    // Use cached layout to avoid reactive dependency
                    if let Some(layout) = cached_layout {
                        log::debug!(
                            "Instrument layout available; scheduling Intro -> Instrument animation"
                        );
                        let from_state = animation_state.get_untracked().now();
                        let to_state =
                            IntroAnimationState::from(IntroAnimationTarget::Instrument(layout));
                        animation_state.set(keyframes![
                            (from_state, 0.0),
                            (to_state, INTRO_TO_INSTRUMENT_MS)
                        ]);
                        set_animation_pair.set(Some((
                            IntroAnimationTarget::Intro,
                            IntroAnimationTarget::Instrument(layout),
                        )));
                        completed.set(false);
                        hidden_svgs.set(false);
                        log::debug!(
                            "Animation pair set to Intro -> Instrument; animation_state keyframes = {}",
                            animation_state.get_untracked().keyframes()
                        );
                        // Let the should_animate effect handle resume
                    } else {
                        log::debug!("Instrument layout not available; cannot start Intro -> Instrument animation");
                    }
                }
                crate::components::NavigationTx::Enter(_)
                    if started.from == RouteId::Play && is_content(started.to) =>
                {
                    log::debug!(
                        "Detected Enter transition from Play -> content (from = {:?}, to = {:?})",
                        started.from,
                        started.to
                    );
                    // Use cached layout to avoid reactive dependency
                    if let Some(layout) = cached_layout {
                        log::debug!(
                            "Instrument layout available; scheduling Instrument -> Intro animation"
                        );
                        let from_state =
                            IntroAnimationState::from(IntroAnimationTarget::Instrument(layout));
                        let to_state = IntroAnimationState::from(IntroAnimationTarget::Intro);
                        animation_state.set(keyframes![
                            (from_state, 0.0),
                            (to_state, INSTRUMENT_TO_INTRO_MS)
                        ]);
                        set_animation_pair.set(Some((
                            IntroAnimationTarget::Instrument(layout),
                            IntroAnimationTarget::Intro,
                        )));
                        completed.set(false);
                        hidden_svgs.set(false);
                        log::debug!(
                            "Animation pair set to Instrument -> Intro; animation_state keyframes = {}",
                            animation_state.get_untracked().keyframes()
                        );
                        // Let the should_animate effect handle resume
                    } else {
                        log::debug!("Instrument layout not available; cannot start Instrument -> Intro animation");
                    }
                }
                _ => {
                    // No matching transition for this navigation state
                }
            }
        }
        // TODO: Tuner transitions (Content <-> Tune) once tuner backend exists
    });

    // ========== Animation Lifecycle Effects ==========

    // Effect: Start/stop animation loop based on animation pair
    // Use a derived signal for cleaner reactivity
    let should_animate = Signal::derive(move || animation_pair().is_some());

    Effect::new({
        let pause = pause.clone();

        move |_| {
            let is_animating = should_animate();

            if is_animating {
                // reset reduced motion tracker each time we start a fresh sequence
                log::debug!("Starting animation sequence; resetting reduced_motion_state");
                reduced_motion_state.set(ReducedMotionState {
                    total_keyframes: animation_state.get_untracked().keyframes(),
                    current_keyframe: 0,
                    accumulated_time: 0.0,
                });
                resume();
            } else {
                log::debug!("No animation_pair present; pausing RAF");
                pause();
            }
        }
    });

    // Effect: Handle animation completion
    Effect::new(move |_| {
        let is_completed = completed();

        if is_completed {
            log::debug!("Completed flag observed true; pausing RAF and finalizing state");
            pause();

            // Use get_untracked to avoid reactive dependency on animation_pair
            let pair = animation_pair.get_untracked();

            // Decide final visibility. Simple heuristic:
            // If last target was Instrument -> hide else show.
            if let Some((_, to)) = pair {
                log::debug!("Final animation target = {:?}", to);
                match to {
                    IntroAnimationTarget::Instrument(_) => {
                        hidden_svgs.set(true);
                        log::debug!("Hiding SVGs because final target is Instrument");
                    }
                    // Tuner path skipped (todo)
                    IntroAnimationTarget::Tuner { .. } => {
                        // future: hidden_svgs.set(true);
                        hidden_svgs.set(true);
                        log::debug!("Hiding SVGs for Tuner path (placeholder behavior)");
                    }
                    IntroAnimationTarget::Intro => {
                        hidden_svgs.set(false);
                        log::debug!("Showing SVGs because final target is Intro");
                    }
                }
            } else {
                log::debug!(
                    "No animation_pair present on completion; leaving SVG visibility unchanged"
                );
            }

            // Clear driving pair so we don't re-trigger
            set_animation_pair.set(None);
        }
    });

    // ========== View Helpers & Rendering ==========

    let now_state = move || animation_state().now();

    let view_box_value = move || {
        let (start, end) = now_state().view_box;
        format!("{} {} {} {}", start.x, start.y, end.x, end.y)
    };

    view! {
        <>
            <Show when=move || !hidden_svgs()>
                <div
                    class="absolute h-screen w-screen overflow-hidden"
                    style:opacity=move || (1.0 - now_state().picture_opacity).to_string()
                >
                    <svg
                        viewBox=view_box_value
                        fill="none"
                        class="stroke-black dark:stroke-red"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <g
                            transform=move || {
                                let (angle, pos) = now_state().strings_rotation;
                                format!("rotate({} {} {})", angle, pos.x, pos.y)
                            }
                            stroke-width=move || now_state().strings_stroke.to_string()
                        >
                            <path d=move || {
                                let (pos_start, pos_end) = now_state().string_1_position;
                                format!(
                                    "M{} {}L{} {}",
                                    pos_start.x,
                                    pos_start.y,
                                    pos_end.x,
                                    pos_end.y,
                                )
                            } />
                            <path d=move || {
                                let (pos_start, pos_end) = now_state().string_2_position;
                                format!(
                                    "M{} {}L{} {}",
                                    pos_start.x,
                                    pos_start.y,
                                    pos_end.x,
                                    pos_end.y,
                                )
                            } />
                        </g>
                    </svg>
                    <svg
                        viewBox=view_box_value
                        fill="none"
                        class="fill-black dark:fill-red"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        {move || {
                            let radius = now_state().sun_radius;
                            let stroke_width = now_state().strings_stroke;
                            now_state()
                                .suns_positions
                                .into_iter()
                                .zip(now_state().suns_splits)
                                .map(|(pos, split)| {
                                    view! {
                                        {if split.x == 0.0 && split.y == 0.0 {
                                            view! { <circle r=radius cx=pos.x cy=pos.y /> }.into_any()
                                        } else {
                                            view! {
                                                <SplitSuns
                                                    pos=pos
                                                    radius=radius
                                                    split=split
                                                    stroke_width=stroke_width
                                                />
                                            }
                                                .into_any()
                                        }}
                                    }
                                })
                                .collect_view()
                        }}
                    </svg>
                </div>
                <div
                    class="absolute h-screen w-screen splash-picture overflow-hidden"
                    style:opacity=move || now_state().picture_opacity.to_string()
                >
                    <Wavering />
                    <svg
                        viewBox="0 0 430 932"
                        class="splash-fragment stone fill-red dark:fill-black stroke-black dark:stroke-red"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <path d=STONE_PATH stroke-width="3" />
                    </svg>
                    <svg
                        viewBox="0 0 430 932"
                        class="splash-fragment siren fill-black dark:fill-red"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <path d=SIREN_PATH_1 />
                        <path d=SIREN_PATH_2 />
                        <path d=SIREN_PATH_3 />
                    </svg>
                    <svg
                        viewBox="0 0 430 932"
                        fill="none"
                        class="splash-fragment flute-shadow stroke-red dark:stroke-black"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <rect
                            x="73.7113"
                            y="576.054"
                            width="53.653"
                            height="8.25253"
                            transform="rotate(-17.1246 48.3365 585.964)"
                            stroke-width="2"
                        />
                    </svg>
                    <svg
                        viewBox="0 0 430 932"
                        class="splash-fragment flute fill-red dark:fill-black stroke-black dark:stroke-red"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <rect
                            width="282.096"
                            height="4.25253"
                            x="48.3365"
                            y="585.964"
                            transform="rotate(-17.1246 48.3365, 585.964)"
                            stroke-width="2"
                        />
                    </svg>
                    <svg
                        viewBox="0 0 430 932"
                        class="splash-fragment sun fill-black dark:fill-red"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <circle r="39" cx="107" cy="164" />
                    </svg>
                    <svg
                        viewBox="0 0 430 932"
                        class="splash-fragment siren-arm fill-black dark:fill-red stroke-red dark:stroke-black"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <path d=SIREN_ARM_PATH stroke-width="2" />
                    </svg>
                    <svg
                        viewBox="0 0 430 932"
                        class="splash-fragment siren-front fill-black dark:fill-red"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        <path d=SIREN_FRONT_PATH_1 />
                        <path d=SIREN_FRONT_PATH_2 />
                        <path d=SIREN_FRONT_PATH_3 />
                        <path d=SIREN_FRONT_PATH_4 />
                        <path d=SIREN_FRONT_PATH_5 />
                    </svg>
                </div>
            </Show>
        </>
    }
}
