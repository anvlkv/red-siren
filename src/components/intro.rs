mod composition;
mod consts;
mod static_path;
// mod suns; // removed as it is currently unused
mod hooks;
mod wavering;

use crate::components::intro::consts::*;
use crate::util::layout_context::{expect_layout_contex, LayoutContextReturn};
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
use leptos::leptos_dom::helpers::set_timeout;

#[component]
pub fn Intro(children: ChildrenFn) -> impl IntoView {
    // Intro: CSS-only animation (unique @keyframes per element). Collapsed → natural layout. Reverse supported via animation-direction.

    // Hidden flag (legacy name preserved for minimal downstream impact)
    let intro_hidden = RwSignal::new(false);

    // Instrument layout (used to size runtime); if absent we delay animation start.
    let LayoutContextReturn {
        space: _space,
        orientation: _orientation,
        left_string_position: _left_string_position,
        right_string_position: _right_string_position,
        key_radius: _key_radius,
        key_band_length: _key_band_length,
        key_band_breadth: _key_band_breadth,
        safe_area_padding: _safe_area_padding,
        key_bands_gap: _key_bands_gap,
        groups_gap: _groups_gap,
        num_keys_per_group: _num_keys_per_group,
        num_groups: _num_groups,
        first_group_channel: _first_group_channel,
        scale: _scale,
        complete_layout,
        iter_keys: _iter_keys,
        key_centers: _key_centers,
        first_key_center: _first_key_center,
        key_pad_main: _key_pad_main,
        key_pad_aux: _key_pad_aux,
    } = expect_layout_contex();

    // Animation state signals (new CSS-based system)
    let keyframes_css = RwSignal::new(String::new());
    let key_styles_signal = RwSignal::new(HashMap::<(u8, u8), String>::new());
    let band_styles_signal = RwSignal::new(HashMap::<(u8, u8), String>::new());
    let left_string_style_signal = RwSignal::new(String::new());
    let right_string_style_signal = RwSignal::new(String::new());
    let picture_style_signal = RwSignal::new(String::new());
    let direction_reverse = RwSignal::new(false);
    let rerun_counter = RwSignal::new(0u32);

    // Initialize animation sequence with dummy snapshot aligned to intro composition.
    // Legacy animation_seq removed (CSS animations now handle progression)

    // Build animations once layout is complete.
    Effect::new(move |_| {
        let layout = complete_layout();
        let num_groups = layout.num_groups.get();
        let num_keys = layout.num_keys_per_group.get();
        // Basic guard: do nothing if zero (should not happen)
        if num_groups == 0 || num_keys == 0 {
            return;
        }
        // DURATIONS
        let forward_ms = INTRO_ANIM_DURATION_MS;
        // Percent -> keyframe markers
        let k1_5 = PHASE_FADE_OVERSHOOT_PCT;
        let k2 = PHASE_COLLAPSE_LINE_PCT;
        let k3 = PHASE_TRAVEL_PCT;
        let k4 = PHASE_REFINE_PCT;
        // Placeholder translations (TODO: compute real collapsed sun & axis offsets)
        // For now all elements translate from (0,0) -> (0,0) so only scale / radius animates.
        let mut css = String::new();
        let mut key_styles = HashMap::new();
        let mut band_styles = HashMap::new();
        // Keys
        //
        // We need to inject per-key collapsed translations (sun + axis) before
        // pushing keyframe strings. The current snippet you provided ends
        // right at the start of the keyframe construction, but the tail of
        // the original `css.push_str(&format!( ... ))` block (with all the
        // percentage frames and closing braces) is not included in the edit
        // window, so I cannot safely rewrite the inner format without the
        // exact trailing lines (the old_text must match exactly).
        //
        // Please provide the remaining lines of this keyframe construction
        // (through the end of the "@keyframes" format for keys and the
        // similar block for bands) so I can replace them with the version
        // that includes:
        //   0%: translate(sun_tx, sun_ty)
        //   {k2}%: translate(axis_tx, axis_ty)
        //   {k3}% / {k4}% / 100%: translate(0,0)
        //
        // Additionally I will add analogous logic for bands (with overshoot)
        // and leave strings rotation-only as requested.
        for (g, k) in layout.iter_keys() {
            // Utilize shared helpers from Layout
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
        // Strings (left/right) - rotation only; no collapsed translation for now
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
        // Picture fade
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
        // Schedule hide (forward only)
        let hide_ms = hide_intro_at_ms();
        let hidden = intro_hidden;
        let dir_flag = direction_reverse;
        set_timeout(
            move || {
                if !dir_flag.get_untracked() {
                    hidden.set(true);
                }
            },
            std::time::Duration::from_millis(hide_ms as u64),
        );
    });

    // Animation direction reactive signal (used to rewrite animation-direction in styles)

    // Derive effective styles by injecting current direction (simple string replace {DIR})
    let key_styles_memo = Memo::new(move |_| {
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        key_styles_signal()
            .into_iter()
            .map(|(k, v)| {
                let replaced = v.replace("{DIR}", dir);
                (k, replaced)
            })
            .collect::<HashMap<_, _>>()
    });
    // removed legacy right_string_transform memo (obsolete)

    let band_styles_memo = Memo::new(move |_| {
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        band_styles_signal()
            .into_iter()
            .map(|(k, v)| {
                let replaced = v.replace("{DIR}", dir);
                (k, replaced)
            })
            .collect::<HashMap<_, _>>()
    });

    // Provide context (non-reactive to per-frame updates)
    let picture_style_memo = Memo::new(move |_| {
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        picture_style_signal().replace("{DIR}", dir)
    });
    let left_string_style_memo = Memo::new(move |_| {
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        left_string_style_signal().replace("{DIR}", dir)
    });
    let right_string_style_memo = Memo::new(move |_| {
        let dir = if direction_reverse.get() {
            "reverse"
        } else {
            "normal"
        };
        right_string_style_signal().replace("{DIR}", dir)
    });
    // Provide new animation context (hooks consume this, see hooks.rs)
    provide_context(crate::components::intro::hooks::IntroAnimationContext {
        key_styles: key_styles_memo,
        band_styles: band_styles_memo,
        left_string_style: left_string_style_memo,
        right_string_style: right_string_style_memo,
        direction_reverse,
        rerun_counter,
    });
    // Collapsed translation computation IMPLEMENTATION ENTRY POINT:
    //
    // We will compute per-key (g,k) final centers using the same math
    // the keyboard component applies implicitly through CSS grid + gaps.
    // From those centers we derive two translation vectors:
    //
    //   sun_tx,  sun_ty  = (sun_center - key_center)
    //   axis_tx, axis_ty = (first_key_center - key_center)
    //
    // These will feed into keyframes:
    //   0%   translate(sun_tx, sun_ty)
    //   {k2}% translate(axis_tx, axis_ty)
    //   {k3}% / {k4}% / 100% translate(0,0)
    //
    // The actual code integrating this lives inside the keyframe
    // generation Effect further above; if that block still shows
    // translate(0px,0px) literals you need to supply the lines of
    // that section so we can patch them precisely (unique line
    // numbers required for replacement in this editing protocol).
    //
    // ACTION NEEDED: Please provide the lines (with numbers) of the
    // keyframe construction (the css.push_str calls for keys/bands)
    // so we can replace the hardcoded (0px,0px) with computed values.

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
    // No RAF loop needed now (CSS handles progression).

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
        <div class="contents">
            // Inject generated keyframes stylesheet once ready
            <Show when=move || !keyframes_css().is_empty()>
                <style inner_html=keyframes_css />
            </Show>
            <Show when=move || !intro_hidden()>
                <IntroComp attr:style=picture_style_memo />
            </Show>
            {move || children()}
        </div>
    }
}
