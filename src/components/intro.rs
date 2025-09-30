mod animation;
mod static_path;
mod suns;
mod wavering;

use keyframe::{keyframes, AnimationSequence};
use leptos::prelude::*;

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

pub use animation::*;

/// Refactored Intro:
/// - Removed external props (`animation`, `on_animation_ended`)
/// - Introduces internal animation pair signal (future: driven by navigation context)
/// - Tuner path intentionally skipped (todo!())
#[component]
pub fn Intro() -> impl IntoView {
    let reduced_motion = use_prefers_reduced_motion();
    log::debug!(
        "Intro component mounted; reduced_motion initial = {}",
        reduced_motion.get_untracked()
    );

    // Internal animation driving signal (None until we detect a route transition Play<->Content)
    let (animation_pair, set_animation_pair) =
        signal::<Option<(IntroAnimationTarget, IntroAnimationTarget)>>(None);
    log::debug!(
        "animation_pair initialized = {:?}",
        animation_pair.get_untracked()
    );

    // determine initial animation state on component launch (default Intro state)
    let animation_state = RwSignal::new({
        let state = IntroAnimationState::default();
        keyframes![(state, 0.0)]
    });
    log::debug!(
        "animation_state initialized; total_keyframes = {}",
        animation_state.get_untracked().keyframes()
    );

    // Reduced motion state bookkeeping
    let reduced_motion_state = RwSignal::new(ReducedMotionState {
        total_keyframes: animation_state.get_untracked().keyframes(),
        current_keyframe: 0,
        accumulated_time: 0.0,
    });
    log::debug!(
        "reduced_motion_state initialized = {:?}",
        reduced_motion_state.get_untracked()
    );

    let completed = RwSignal::new(false);
    log::debug!("completed initialized = {}", completed.get_untracked());

    // Whether decorative SVGs are currently hidden (after reaching Instrument view)
    let hidden_svgs = RwSignal::new(false);
    log::debug!("hidden_svgs initialized = {}", hidden_svgs.get_untracked());

    // Navigation context (nav_tx + early started payload)
    let nav_tx = expect_context::<Signal<Option<crate::components::NavigationTx>>>();
    let nav_started = expect_context::<Signal<Option<NavStartedPayload>>>();
    log::debug!(
        "navigation contexts obtained; nav_tx = {:?}, nav_started = {:?}",
        nav_tx.get_untracked(),
        nav_started.get_untracked()
    );

    // Instrument layout resource (invoke/event share the same name)
    let UseTauriResourceReturn {
        data: instrument_layout,
        ..
    } = use_tauri_resource::<shared::instrument::Layout>(shared::instrument::events::LAYOUT);
    log::debug!(
        "instrument_layout resource hook initialized; current = {:?}",
        instrument_layout.get_untracked()
    );

    fn is_content(r: RouteId) -> bool {
        matches!(
            r,
            RouteId::Home | RouteId::About | RouteId::Donate | RouteId::Permissions
        )
    }

    let Pausable { pause, resume, .. } = use_raf_fn_with_options(
        move |UseRafFnCallbackArgs { delta, .. }| {
            log::trace!("RAF callback tick; delta = {}", delta);
            let mut state = animation_state();
            log::trace!(
                "RAF state before tick: time = {}, duration = {}, keyframes = {}",
                state.time(),
                state.duration(),
                state.keyframes()
            );
            let remainder = if reduced_motion() {
                log::trace!("Reduced motion path taken in RAF");
                let mut rm_state = reduced_motion_state();
                rm_state.accumulated_time += delta;
                log::trace!(
                    "Reduced motion accumulated_time updated = {}",
                    rm_state.accumulated_time
                );

                // Get the current keyframe pair (current, next)
                let (_, next_kf) = state.pair();

                if let Some(next_kf) = next_kf {
                    log::trace!("Next keyframe present with time = {}", next_kf.time());
                    // If we've accumulated enough time to reach the next keyframe
                    if rm_state.accumulated_time >= next_kf.time() {
                        // Jump directly to the next keyframe time
                        let remainder = state.advance_to(next_kf.time());
                        rm_state.current_keyframe += 1;
                        animation_state.set(state);
                        reduced_motion_state.set(rm_state);
                        log::debug!(
                            "Advanced to keyframe; current_keyframe = {}, remainder = {:?}",
                            reduced_motion_state.get_untracked().current_keyframe,
                            remainder
                        );
                        Some(remainder)
                    } else {
                        // Not enough time accumulated yet, just update state
                        reduced_motion_state.set(rm_state);
                        log::trace!(
                            "Not enough accumulated time for next keyframe; accumulated = {}",
                            reduced_motion_state.get_untracked().accumulated_time
                        );
                        None
                    }
                } else {
                    // No next keyframe (animation finished)
                    reduced_motion_state.set(rm_state);
                    log::debug!("No next keyframe; reduced motion animation finished");
                    None
                }
            } else {
                log::trace!("Normal motion path taken in RAF");
                let rem = state.duration() - state.time();
                let remainder = state.advance_by(delta.min(rem));
                animation_state.set(state);
                log::debug!(
                    "Advanced by {}; remainder = {:?}",
                    delta.min(rem),
                    remainder
                );
                Some(remainder)
            };

            if remainder.is_some_and(|r| r <= 0.0) {
                completed.set(true);
                log::debug!("Animation marked completed (remainder <= 0)");
            }
        },
        UseRafFnOptions::default().immediate(false),
    );

    // Decide when to trigger Intro ↔ Instrument animations (using nav_tx + started payload)
    Effect::new({
        let resume = resume.clone();
        move |_| {
            let started_opt = nav_started();
            let nav_state = nav_tx();

            log::trace!(
                "Nav effect triggered; nav_started = {:?}, nav_tx = {:?}, animation_pair = {:?}",
                nav_started.get_untracked(),
                nav_tx.get_untracked(),
                animation_pair.get_untracked()
            );

            let _interrupted = animation_pair().is_some();
            if _interrupted {
                log::debug!(
                    "Animation was interrupted; current pair = {:?}",
                    animation_pair.get_untracked()
                );
            }

            if let Some(started) = started_opt {
                log::debug!("Navigation started payload = {:?}", started);
                match nav_state {
                    Some(crate::components::NavigationTx::Leave(_))
                        if is_content(started.from) && started.to == RouteId::Play =>
                    {
                        log::debug!(
                                "Detected Leave transition from content -> Play (from = {:?}, to = {:?})",
                                started.from,
                                started.to
                            );
                        if let Some(layout) = instrument_layout() {
                            log::debug!("Instrument layout available; scheduling Intro -> Instrument animation");
                            let from_state = animation_state().now();
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
                            resume();
                        } else {
                            log::debug!("Instrument layout not available; cannot start Intro -> Instrument animation");
                        }
                    }
                    Some(crate::components::NavigationTx::Enter(_))
                        if started.from == RouteId::Play && is_content(started.to) =>
                    {
                        log::debug!(
                                "Detected Enter transition from Play -> content (from = {:?}, to = {:?})",
                                started.from,
                                started.to
                            );
                        if let Some(layout) = instrument_layout() {
                            log::debug!("Instrument layout available; scheduling Instrument -> Intro animation");
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
                            resume();
                        } else {
                            log::debug!("Instrument layout not available; cannot start Instrument -> Intro animation");
                        }
                    }
                    _ => {
                        log::trace!(
                            "Nav effect: no matching transition detected (nav_state = {:?})",
                            nav_state
                        );
                    }
                }
            } else {
                log::trace!("Nav effect: nav_started is None");
            }
            // TODO: Tuner transitions (Content <-> Tune) once tuner backend exists
        }
    });

    // Start/stop animation loop based on internal pair
    Effect::new({
        let pause = pause.clone();

        move |_| {
            log::trace!(
                "Start/stop effect triggered; animation_pair = {:?}",
                animation_pair.get_untracked()
            );
            if animation_pair().is_some() {
                // reset reduced motion tracker each time we start a fresh sequence
                log::debug!("Starting animation sequence; resetting reduced_motion_state");
                reduced_motion_state.set(ReducedMotionState {
                    total_keyframes: animation_state.get_untracked().keyframes(),
                    current_keyframe: 0,
                    accumulated_time: 0.0,
                });
                log::trace!(
                    "reduced_motion_state after reset = {:?}",
                    reduced_motion_state.get_untracked()
                );
                resume();
            } else {
                log::debug!("No animation_pair present; pausing RAF");
                pause();
            }
        }
    });

    // Stop RAF when sequence finished, hide / show SVGs appropriately
    Effect::new(move |_| {
        if completed() {
            log::debug!("Completed flag observed true; pausing RAF and finalizing state");
            pause();
            // Decide final visibility. Simple heuristic:
            // If last target was Instrument -> hide else show.
            if let Some((_, to)) = animation_pair() {
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
            log::trace!("Cleared animation_pair after completion");
        }
    });

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
