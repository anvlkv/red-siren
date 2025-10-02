mod animation;

mod static_path;
// mod suns; // removed as it is currently unused
mod wavering;

use crate::{
    components::intro::animation::IntroTransformSnapshot,
    util::layout_context::{expect_layout_contex, LayoutContextReturn},
};
use keyframe::{keyframes, AnimationSequence};
use leptos::prelude::*;

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

pub struct IntroContext {
    // --- Instrument properties ---
    pub left_string_style: Memo<String>,
    pub right_string_style: Memo<String>,
    pub key_styles: Memo<HashMap<(u8, u8), String>>,
    pub band_styles: Memo<HashMap<(u8, u8), String>>,
    // --- Tuner properties ---
    // TODO: add properties with tuner
}

#[component]
pub fn Intro(children: ChildrenFn) -> impl IntoView {
    // Intro animation now uses a pure tweenable snapshot (IntroTransformSnapshot)
    // instead of an internal runtime/timeline.
    //
    // We keep:
    // - instrument_layout (when available)
    // - Option<IntroTransformSnapshot> for the current state
    // - a top-level CSS var string (intro_vars) rebuilt on RAF
    // - static transform style templates referencing primitive CSS vars
    //
    // Future: external keyframe sequencing will interpolate snapshots.
    // Currently we emit the baseline (identity) snapshot once per RAF.
    //
    // Translation (tx/ty) remains 0 until real layout mapping is integrated.

    let hidden_svgs = RwSignal::new(false);

    // Instrument layout (used to size runtime); if absent we delay animation start.
    let LayoutContextReturn {
        space,
        orientation,
        left_string_position,
        right_string_position,
        key_radius,
        key_band_length,
        key_band_breadth,
        safe_area_padding,
        key_bands_gap,
        groups_gap,
        num_keys_per_group,
        num_groups,
        first_group_channel,
        scale,
        complete_layout,
    } = expect_layout_contex();

    // Snapshot (None when idle)
    let animation_seq = RwSignal::<AnimationSequence<IntroTransformSnapshot>>::new(keyframes![(
        IntroTransformSnapshot::default(),
        0.0
    )]);

    let intro_vars = Signal::derive(move || {
        animation_seq()
            .now_strict()
            .map(|s| s.emit_css_vars())
            .unwrap_or_default()
    });

    // Whether transform style templates are currently mounted (animation running)
    let (transform_templates_enabled, set_transform_templates_enabled) = signal(false);

    // Build key style templates (only when layout changes OR templates toggled)
    let key_styles = Memo::new(move |old| {
        let mut map = HashMap::new();
        if !transform_templates_enabled() {
            return old.cloned().unwrap_or(map);
        }
        let num_groups = num_groups();
        let num_keys_per_group = num_keys_per_group();

        for g in 0..num_groups {
            for k in 0..num_keys_per_group {
                map.insert(
                        (g, k),
                        format!(
                            "transform: translate(var(--key-{g}-{k}-tx,0px), var(--key-{g}-{k}-ty,0px)) rotate(var(--key-{g}-{k}-rot,0deg)) scale(var(--key-{g}-{k}-sx,1), var(--key-{g}-{k}-sy,1));"
                        ),
                    );
            }
        }
        map
    });

    // Band style templates
    let band_styles = Memo::new(move |old| {
        let mut map = HashMap::new();
        if !transform_templates_enabled() {
            return old.cloned().unwrap_or(map);
        }
        let num_groups = num_groups();
        let num_keys_per_group = num_keys_per_group();
        for g in 0..num_groups {
            for k in 0..num_keys_per_group {
                map.insert(
                        (g, k),
                        format!(
                            "transform: translate(var(--band-{g}-{k}-tx,0px), var(--band-{g}-{k}-ty,0px)) rotate(var(--band-{g}-{k}-rot,0deg)) scale(var(--band-{g}-{k}-sx,1), var(--band-{g}-{k}-sy,1)); border-radius: calc(var(--band-{g}-{k}-round,0) * 9999px);"
                        ),
                    );
            }
        }
        map
    });

    // Strings (only left for now; right mirrors left if desired)
    let left_string_style = Memo::new(move |_| {
        if transform_templates_enabled() {
            "transform: translate(var(--left-string-tx,0px), var(--left-string-ty,0px)) rotate(var(--left-string-rot,0deg)) scale(var(--left-string-sx,1), var(--left-string-sy,1));"
                    .to_string()
        } else {
            String::new()
        }
    });
    let right_string_style = Memo::new(move |_| {
        // Placeholder (same as left or empty)
        if transform_templates_enabled() {
            "transform: translate(var(--right-string-tx,0px), var(--right-string-ty,0px)) rotate(var(--right-string-rot,0deg)) scale(var(--right-string-sx,1), var(--right-string-sy,1));".to_string()
        } else {
            String::new()
        }
    });

    // Provide context (non-reactive to per-frame updates)
    provide_context(IntroContext {
        left_string_style,
        right_string_style,
        key_styles,
        band_styles,
    });

    // Start animation automatically once layout arrives (placeholder trigger)
    Effect::new(move |_| {
        // TODO: use navigation to determine animation start.
        // let layout = complete_layout();
        // let snap = IntroTransformSnapshot::new(layout);
        // // Store snapshot
        // // snapshot.set(Some(snap.clone()));
        // // Emit initial CSS vars immediately
        // set_intro_vars.set(snap.emit_css_vars());
        // set_transform_templates_enabled.set(true);
        // hidden_svgs.set(false);
        //
        //
        // TODO: determine actual keyframes
        //
        // animation_seq.set(keyframes![(kf_1, 0.0), (kf_2, 1000.0, EaseOut)])
    });

    // RAF driver
    let _raf = use_raf_fn_with_options(
        {
            move |UseRafFnCallbackArgs { delta, .. }| {
                animation_seq.update(|s| {
                    let rem = s.duration() - s.time();
                    let clamp_d = delta.min(rem);
                    s.advance_by(clamp_d);
                });
            }
        },
        UseRafFnOptions::default().immediate(true),
    );

    // // ========== External Context & Resources ==========

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

    // // Initialize correct animation state based on current route
    // // Track instrument_layout so we update when it arrives

    //     log::debug!(
    //         "Intro state check: route = {:?}, layout available = {}",
    //         current_route,
    //         layout.is_some()
    //     );

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
        <div class="contents" style=intro_vars>
            <Show when=move || !hidden_svgs()>
                <div
                    class="absolute h-screen w-screen splash-picture overflow-hidden"
                    // style:opacity=move || now_state().picture_opacity.to_string()
                    role="img"
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
            {move || children()}
        </div>
    }
}
