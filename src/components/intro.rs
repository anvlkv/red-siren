mod composition;
mod consts;
mod static_path;
mod wavering;

use composition::IntroComp;
use leptos::prelude::*;

use composition::IntroComp;
use leptos_use::{use_raf_fn_with_options, UseRafFnCallbackArgs, UseRafFnOptions};
use static_path::*;

use wavering::*;

use crate::util::{
    animation::ReducedMotionState,
    tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};
use common::RouteId;
use std::str::FromStr;

use std::collections::HashMap;

use composition::IntroComp;

#[component]
pub fn Intro(
    #[prop(into)] nav_started: Signal<Option<NavStartedPayload>>,
    #[prop(into)] current_route: Signal<RouteId>,
    children: ChildrenFn,
) -> impl IntoView {
    let is_intro_fading = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(NavStartedPayload { to, from, .. }) = nav_started() {
            let is_exit = from.is_content() && !to.is_content();

            log::debug!("intro: nav_started: to={to:?}, is_exit={is_exit}");
            is_intro_fading.set(is_exit);
        }
    });

    Effect::new(move |_| {
        // Direct props
        let nav_tx_val = nav_tx();
        let started_opt = nav_started();
        // If navigation start payload absent we stay in current static/animated state
        let Some(started) = started_opt else {
            return;
        };
        // Local (non-static) guard for redundant triggering
        // Fetch last applied state (tx_id, reverse_flag) once per effect run
        let last_current = last_applied.get();

        // Determine directional intent

    // Navigation handled client-side (use_location + prev path)

    // Current location for initial state
    let location = use_location();
    // Previous path to compute direction and from/to
    let (prev_path, set_prev_path) = signal(String::from("/"));

    // // Instrument layout resource (fetched via invoke/event)
    // let UseTauriResourceReturn {
    //     data: instrument_layout,
    //     ..
    // } = use_tauri_resource::<shared::instrument::Layout>(shared::instrument::events::LAYOUT);

    // // ========== Helper Functions ==========

    // fn is_content(r: RouteId) -> bool {
    //     matches!(
    //         r,
    //         RouteId::Home | RouteId::About | RouteId::Donate | RouteId::Permissions
    //     )
    // }

    // // ========== Initial State Setup ==========

        // last_current already captured above

        match nav_tx_val {
            // Forward: leaving content (we only animate when destination is Play/Tune and we currently show intro)
            Some(crate::components::NavigationTx::Leave(tx_id)) if (to_play || to_tune) => {
                if last_current != Some((tx_id, false)) {
                    direction_reverse.set(false); // forward direction
                    intro_hidden.set(false); // ensure visible while animating
                    transition_active.set(true); // enable element styles
                    rerun_counter.update(|c| *c = c.wrapping_add(1)); // restart animations
                    last_applied.set(Some((tx_id, false)));

    //     // Only initialize if we haven't already animated to this state
    //     if current_route == RouteId::Play {
    //         if let Some(layout) = layout {
    //             // Check if we're already in the correct state
    //             if animation_pair.get_untracked().is_none() && !completed.get_untracked() {
    //                 log::debug!("Initializing Intro in Instrument state for Play route");
    //                 let target_state =
    //                     IntroAnimationState::from(IntroAnimationTarget::Instrument(layout));
    //                 animation_state.set(keyframes![(target_state, 0.0)]);
    //                 hidden_svgs.set(true);
    //                 completed.set(true);
    //             }
    //         } else {
    //             log::debug!("On Play route but instrument layout not yet available");
    //         }
    //     } else if current_route == RouteId::Tune {
    //         // TODO: Handle Tune route once backend is ready
    //         log::debug!("Tune route initialization not yet implemented");
    //     } else if is_content(current_route) {
    //         // Only set if not already in correct state
    //         if animation_pair.get_untracked().is_none() && !completed.get_untracked() {
    //             hidden_svgs.set(false);
    //             completed.set(true);
    //         }
    //     }
    // });

    // // ========== Animation Loop (RAF) ==========

    // let Pausable { pause, resume, .. } = use_raf_fn_with_options(
    //     move |UseRafFnCallbackArgs { delta, .. }| {
    //         let mut state = animation_state.get_untracked();
    //         // Variables to track final state for completion check
    //         let mut final_time = state.time();
    //         let mut final_duration = state.duration();

    //         // Skip processing if delta is 0 (first frame or paused)
    //         if delta == 0.0 {
    //             return;
    //         }

    //         let _remainder = if reduced_motion.get_untracked() {
    //             let mut rm_state = reduced_motion_state.get_untracked();
    //             rm_state.accumulated_time += delta;

    //             // Get the current keyframe pair (current, next)
    //             let (_, next_kf) = state.pair();
    //             if let Some(next_kf) = next_kf {
    //                 // If we've accumulated enough time to reach the next keyframe
    //                 if rm_state.accumulated_time >= next_kf.time() {
    //                     // Jump directly to the next keyframe time
    //                     let remainder = state.advance_to(next_kf.time());
    //                     rm_state.current_keyframe += 1;
    //                     // Store values before moving state
    //                     final_time = state.time();
    //                     final_duration = state.duration();
    //                     animation_state.set(state);
    //                     reduced_motion_state.set(rm_state);
    //                     log::debug!(
    //                         "Advanced to keyframe; current_keyframe = {}, remainder = {:?}",
    //                         rm_state.current_keyframe,
    //                         remainder
    //                     );
    //                     Some(remainder)
    //                 } else {
    //                     // Not enough time accumulated yet, just update state
    //                     reduced_motion_state.set(rm_state);
    //                     None
    //                 }
    //             } else {
    //                 // No next keyframe (animation finished)
    //                 reduced_motion_state.set(rm_state);
    //                 log::debug!("No next keyframe; reduced motion animation finished");
    //                 None
    //             }
    //         } else {
    //             let rem = state.duration() - state.time();
    //             let remainder = state.advance_by(delta.min(rem));
    //             // Store values before moving state
    //             final_time = state.time();
    //             final_duration = state.duration();
    //             animation_state.set(state);
    //             Some(remainder)
    //         };

    //         // Mark completed only if animation truly finished (reached duration with actual progress)
    //         if final_time >= final_duration && final_duration > 0.0 && delta > 0.0 {
    //             completed.set(true);
    //             log::debug!(
    //                 "Animation marked completed (time {} >= duration {})",
    //                 final_time,
    //                 final_duration
    //             );
    //         }
    //     },
    //     UseRafFnOptions::default().immediate(false),
    // );

    // ========== Navigation Transition Logic (client-side) ==========
    // Drive Intro animations by comparing previous and current routes.
    Effect::new(move |_| {
        let current = location.pathname.get();
        let from = RouteId::from_str(&prev_path.get_untracked()).unwrap_or(RouteId::Home);
        let to = RouteId::from_str(&current).unwrap_or(RouteId::Home);

    //     // Cache the instrument layout to avoid multiple reactive accesses
    //     let cached_layout = instrument_layout.get_untracked();

    //     let _interrupted = animation_pair.get_untracked().is_some();
    //     if _interrupted {
    //         log::debug!(
    //             "Animation was interrupted; current pair = {:?}",
    //             animation_pair.get_untracked()
    //         );
    //     }

        if is_content(from) && to == RouteId::Play {
            log::debug!(
                "Detected transition content -> Play (from = {:?}, to = {:?})",
                from,
                to
            );
            if let Some(layout) = cached_layout {
                let from_state = animation_state.get_untracked().now();
                let to_state = IntroAnimationState::from(IntroAnimationTarget::Instrument(layout));
                animation_state.set(animation_keyframes::intro_to_instrument(
                    from_state, to_state,
                ));
                set_animation_pair.set(Some((
                    IntroAnimationTarget::Intro,
                    IntroAnimationTarget::Instrument(layout),
                )));
                completed.set(false);
                hidden_svgs.set(false);
            } else {
                log::debug!(
                    "Instrument layout not available; cannot start Intro -> Instrument animation"
                );
            }
        } else if from == RouteId::Play && is_content(to) {
            log::debug!(
                "Detected transition Play -> content (from = {:?}, to = {:?})",
                from,
                to
            );
            if let Some(layout) = cached_layout {
                let from_state =
                    IntroAnimationState::from(IntroAnimationTarget::Instrument(layout));
                let to_state = IntroAnimationState::from(IntroAnimationTarget::Intro);
                animation_state.set(animation_keyframes::instrument_to_intro(
                    from_state, to_state,
                ));
                set_animation_pair.set(Some((
                    IntroAnimationTarget::Instrument(layout),
                    IntroAnimationTarget::Intro,
                )));
                completed.set(false);
                hidden_svgs.set(false);
            } else {
                log::debug!(
                    "Instrument layout not available; cannot start Instrument -> Intro animation"
                );
            }
        }
        // Update previous path after handling transition
        set_prev_path.set(current);
        // TODO: Tuner transitions (Content <-> Tune) once tuner backend exists
    });

    // // ========== Animation Lifecycle Effects ==========

    // // Effect: Start/stop animation loop based on animation pair
    // // Use a derived signal for cleaner reactivity
    // let should_animate = Signal::derive(move || animation_pair().is_some());

    // Effect::new({
    //     let pause = pause.clone();

    //     move |_| {
    //         let is_animating = should_animate();

    //         if is_animating {
    //             // reset reduced motion tracker each time we start a fresh sequence
    //             log::debug!("Starting animation sequence; resetting reduced_motion_state");
    //             reduced_motion_state.set(ReducedMotionState {
    //                 total_keyframes: animation_state.get_untracked().keyframes(),
    //                 current_keyframe: 0,
    //                 accumulated_time: 0.0,
    //             });
    //             resume();
    //         } else {
    //             log::debug!("No animation_pair present; pausing RAF");
    //             pause();
    //         }
    //     }
    // });

    // // Effect: Handle animation completion
    // Effect::new(move |_| {
    //     let is_completed = completed();

    //     if is_completed {
    //         log::debug!("Completed flag observed true; pausing RAF and finalizing state");
    //         pause();

    //         // Use get_untracked to avoid reactive dependency on animation_pair
    //         let pair = animation_pair.get_untracked();

    //         // Decide final visibility. Simple heuristic:
    //         // If last target was Instrument -> hide else show.
    //         if let Some((_, to)) = pair {
    //             log::debug!("Final animation target = {:?}", to);
    //             match to {
    //                 IntroAnimationTarget::Instrument(_) => {
    //                     hidden_svgs.set(true);
    //                     log::debug!("Hiding SVGs because final target is Instrument");
    //                 }
    //                 // Tuner path skipped (todo)
    //                 IntroAnimationTarget::Tuner { .. } => {
    //                     // future: hidden_svgs.set(true);
    //                     hidden_svgs.set(true);
    //                     log::debug!("Hiding SVGs for Tuner path (placeholder behavior)");
    //                 }
    //                 IntroAnimationTarget::Intro => {
    //                     hidden_svgs.set(false);
    //                     log::debug!("Showing SVGs because final target is Intro");
    //                 }
    //             }
    //         } else {
    //             log::debug!(
    //                 "No animation_pair present on completion; leaving SVG visibility unchanged"
    //             );
    //         }

    //         // Clear driving pair so we don't re-trigger
    //         set_animation_pair.set(None);
    //     }
    // });

    // // ========== View Helpers & Rendering ==========

    // let now_state = move || animation_state().now();

    // Top-level style string providing primitive CSS vars (empty when idle)

    view! {
        <div class="contents">
            <IntroComp exit=is_intro_fading />
            {move || children()}
        </div>
    }
}
