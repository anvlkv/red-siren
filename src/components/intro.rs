mod composition;
mod consts;
mod hooks;
mod static_path;
mod wavering;

use crate::components::intro::consts::*;
use crate::util::layout_context::{expect_layout_contex, LayoutContextReturn};
use leptos::leptos_dom::helpers::set_timeout;
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
    #[prop(into)] nav_tx: Signal<Option<crate::components::NavigationTx>>,
    #[prop(into)] nav_started: Signal<Option<NavStartedPayload>>,
    #[prop(into)] current_route: Signal<RouteId>,
    children: ChildrenFn,
) -> impl IntoView {
    // Intro animation component (controlled):
    //   - All navigation signals are required props (no context fallbacks).
    //   - Static content routes: intro visible, no animations.
    //   - Forward: content -> Play/Tune (Leave tx toward Play/Tune).
    //   - Reverse: Play/Tune -> content (Enter tx from Play/Tune).

    // Visibility flag (hidden only after successful forward transition completes).
    let intro_hidden = RwSignal::new(false);

    // Layout context (we only need complete_layout + iter helpers here).
    let LayoutContextReturn {
        complete_layout, ..
    } = expect_layout_contex();

    // Animation state signals (CSS-driven)
    let keyframes_css = RwSignal::new(String::new());
    let key_styles_signal = RwSignal::new(HashMap::<(u8, u8), String>::new());
    let band_styles_signal = RwSignal::new(HashMap::<(u8, u8), String>::new());
    let left_string_style_signal = RwSignal::new(String::new());
    let right_string_style_signal = RwSignal::new(String::new());
    let picture_style_signal = RwSignal::new(String::new());
    let direction_reverse = RwSignal::new(false);
    let rerun_counter = RwSignal::new(0u32);
    // Active only while a forward or reverse transition animation is running.
    // For content routes (None/Home/About/Donate/Permissions) this stays false so
    // the IntroComp renders statically with no fade (animation.md: intro comp load -> home).
    let transition_active = RwSignal::new(false);
    // Track last applied navigation animation (tx_id, reverse_flag)
    let last_applied = RwSignal::new(None::<(u64, bool)>);

    // Build keyframes & per-element animation styles once layout is available or changes.
    Effect::new(move |_| {
        let layout = complete_layout();
        let num_groups = layout.num_groups.get();
        let num_keys = layout.num_keys_per_group.get();
        if num_groups == 0 || num_keys == 0 {
            return;
        }

        let forward_ms = INTRO_ANIM_DURATION_MS;
        let k1_5 = PHASE_FADE_OVERSHOOT_PCT;
        let k2 = PHASE_COLLAPSE_LINE_PCT;
        let k3 = PHASE_TRAVEL_PCT;
        let k4 = PHASE_REFINE_PCT;

        let mut css = String::new();
        let mut key_styles = HashMap::new();
        let mut band_styles = HashMap::new();

        // Iterate keys
        for (g, k) in layout.iter_keys() {
            let (final_cx, final_cy) = layout.key_center(g, k);
            let (first_cx, first_cy) = layout.first_key_center();
            let sun_cx = (INTRO_SUN_CX / INTRO_ART_WIDTH) * layout.space.x;
            let sun_cy = (INTRO_SUN_CY / INTRO_ART_HEIGHT) * layout.space.y;
            let sun_tx = sun_cx - final_cx;
            let sun_ty = sun_cy - final_cy;
            let axis_tx = first_cx - final_cx;
            let axis_ty = first_cy - final_cy;
            let fmt = |v: f32| {
                if (v.fract()).abs() < 0.0005 {
                    format!("{:.0}", v)
                } else {
                    format!("{:.3}", v)
                }
            };

            // Key animation
            let anim_name = format!("key-anim-g-{g}-k-{k}");
            css.push_str(&format!(
                "@keyframes {name}{{\
 0%{{transform:translate({stx}px,{sty}px) scale({ks});}}\
 {k2}%{{transform:translate({atx}px,{aty}px) scale({mid});}}\
 {k3}%{{transform:translate(0px,0px) scale(1);}}\
 {k4}%{{transform:translate(0px,0px) scale(1);}}\
 100%{{transform:translate(0px,0px) scale(1);}}}}",
                name = anim_name,
                ks = KEY_START_SCALE,
                mid = (KEY_START_SCALE + 1.0) * 0.5,
                k2 = k2,
                k3 = k3,
                k4 = k4,
                stx = fmt(sun_tx),
                sty = fmt(sun_ty),
                atx = fmt(axis_tx),
                aty = fmt(axis_ty)
            ));
            let style = format!(
                "animation:{name} {dur}ms {ease} 1 both;animation-direction:{{DIR}};",
                name = anim_name,
                dur = forward_ms,
                ease = EASE_MAIN
            );
            key_styles.insert((g, k), style);

            // Band animation
            let band_anim = format!("band-anim-g-{g}-k-{k}");
            css.push_str(&format!(
                "@keyframes {name}{{\
 0%{{transform:translate({stx}px,{sty}px) scale({bs});border-radius:{brc}px;}}\
 {k1_5}%{{transform:translate({stx}px,{sty}px) scale({bo});border-radius:{brc}px;}}\
 {k2}%{{transform:translate({atx}px,{aty}px) scale({bc});border-radius:{brc}px;}}\
 {k3}%{{transform:translate(0px,0px) scale({bf});border-radius:{brc}px;}}\
 {k4}%{{transform:translate(0px,0px) scale({bf});border-radius:0px;}}\
 100%{{transform:translate(0px,0px) scale({bf});border-radius:0px;}}}}",
                name = band_anim,
                bs = BAND_START_SCALE,
                bo = BAND_OVERSHOOT_SCALE,
                bc = BAND_AXIS_COLLAPSE_SCALE,
                bf = BAND_REFINE_SCALE,
                brc = BAND_CIRCLE_RADIUS_PX,
                k1_5 = k1_5,
                k2 = k2,
                k3 = k3,
                k4 = k4,
                stx = fmt(sun_tx),
                sty = fmt(sun_ty),
                atx = fmt(axis_tx),
                aty = fmt(axis_ty)
            ));
            let bstyle = format!(
                "animation:{name} {dur}ms {ease} 1 both;animation-direction:{{DIR}};",
                name = band_anim,
                dur = forward_ms,
                ease = EASE_MAIN
            );
            band_styles.insert((g, k), bstyle);
        }

        // Strings (rotation only)
        css.push_str(&format!(
            "@keyframes string-anim-left{{0%{{transform:translate(0px,0px) rotate({start}deg);}}\
 {k2}%{{transform:translate(0px,0px) rotate({mid}deg);}}\
 {k3}%{{transform:translate(0px,0px) rotate(0deg);}}\
 100%{{transform:translate(0px,0px) rotate(0deg);}}}}",
            start = STRING_START_ROT_DEG,
            mid = STRING_START_ROT_DEG * 0.35,
            k2 = k2,
            k3 = k3
        ));
        css.push_str(&format!(
            "@keyframes string-anim-right{{0%{{transform:translate(0px,0px) rotate({start}deg);}}\
 {k2}%{{transform:translate(0px,0px) rotate({mid}deg);}}\
 {k3}%{{transform:translate(0px,0px) rotate(0deg);}}\
 100%{{transform:translate(0px,0px) rotate(0deg);}}}}",
            start = STRING_START_ROT_DEG,
            mid = STRING_START_ROT_DEG * 0.35,
            k2 = k2,
            k3 = k3
        ));
        let str_style_left = format!(
            "animation:string-anim-left {dur}ms {ease} 1 both;animation-direction:{{DIR}};",
            dur = forward_ms,
            ease = EASE_MAIN
        );
        let str_style_right = format!(
            "animation:string-anim-right {dur}ms {ease} 1 both;animation-direction:{{DIR}};",
            dur = forward_ms,
            ease = EASE_MAIN
        );

        // Picture fade (forward only; reverse keeps hidden until we re-show explicitly)
        css.push_str(&format!(
            "@keyframes intro-picture-fade{{0%{{opacity:1;}}{k1_5}%{{opacity:.25;}}{k2}%{{opacity:0;}}100%{{opacity:0;}}}}",
            k1_5 = k1_5,
            k2 = k2
        ));
        let picture_anim = format!(
            "animation:intro-picture-fade {}ms {} 1 forwards;animation-direction:{{DIR}};",
            percent_to_time_ms(PICTURE_FADE_OUT_PCT),
            EASE_FADE
        );

        // Commit signals
        keyframes_css.set(css);
        key_styles_signal.set(key_styles);
        band_styles_signal.set(band_styles);
        left_string_style_signal.set(str_style_left);
        right_string_style_signal.set(str_style_right);
        picture_style_signal.set(picture_anim);
    });

    // Direction-aware style resolution
    let key_styles_memo = Memo::new(move |_| {
        if !transition_active.get() {
            return HashMap::new();
        }
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        key_styles_signal()
            .into_iter()
            .map(|(k, v)| (k, v.replace("{DIR}", dir)))
            .collect::<HashMap<_, _>>()
    });
    let band_styles_memo = Memo::new(move |_| {
        if !transition_active.get() {
            return HashMap::new();
        }
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        band_styles_signal()
            .into_iter()
            .map(|(k, v)| (k, v.replace("{DIR}", dir)))
            .collect::<HashMap<_, _>>()
    });
    let picture_style_memo = Memo::new(move |_| {
        if !transition_active.get() {
            // Static (no active transition): keep picture fully visible with explicit style.
            "opacity:1;".to_string()
        } else {
            let dir = if direction_reverse.get() {
                "reverse"
            } else {
                "normal"
            };
            // Always include a concrete style (opacity baseline) plus animation definition.
            format!("opacity:1;{}", picture_style_signal().replace("{DIR}", dir))
        }
    });
    let left_string_style_memo = Memo::new(move |_| {
        if !transition_active.get() {
            return String::new();
        }
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        left_string_style_signal().replace("{DIR}", dir)
    });
    let right_string_style_memo = Memo::new(move |_| {
        if !transition_active.get() {
            return String::new();
        }
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        right_string_style_signal().replace("{DIR}", dir)
    });

    // Provide context to intro subcomponents
    provide_context(crate::components::intro::hooks::IntroAnimationContext {
        key_styles: key_styles_memo,
        band_styles: band_styles_memo,
        left_string_style: left_string_style_memo,
        right_string_style: right_string_style_memo,
        direction_reverse,
        rerun_counter,
    });

    // Initial static route handling (no animation) based on current_route prop.
    // Instrument / tuner start hidden; other routes visible & idle.
    Effect::new(move |_| match current_route() {
        RouteId::Play | RouteId::Tune => {
            intro_hidden.set(true);
            transition_active.set(false);
        }
        _ => {
            intro_hidden.set(false);
            transition_active.set(false);
        }
    });

    // Navigation-driven animation triggering (forward & reverse) – props only (no legacy context)
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
            <Show when=move || !keyframes_css().is_empty()>
                <style inner_html=keyframes_css />
            </Show>
            <Show when=move || !intro_hidden()>
                <IntroComp attr:style=picture_style_memo />
            </Show>
            <Show when=|| {
                cfg!(debug_assertions)
            }>
                {move || {
                    view! {
                        <div class="fixed left-2 top-2 z-[9999] px-2 py-1 rounded bg-black/60 text-[10px] font-mono tracking-wide text-white pointer-events-none select-none">
                            {if transition_active() {
                                "intro transition: active"
                            } else {
                                "intro transition: idle"
                            }}
                        </div>
                    }
                }}
            </Show>

            {move || children()}
        </div>
    }
}
