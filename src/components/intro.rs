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

use crate::util::animation::ReducedMotionState;

pub use animation::*;

#[component]
pub fn Intro(
    /// animate transition `from` one state `to` another
    #[prop(optional)]
    animation: Signal<Option<(IntroAnimationTarget, IntroAnimationTarget)>>,
    /// notify `callback` on animation end
    #[prop(optional, into)]
    on_animation_ended: Option<Callback<()>>,
) -> impl IntoView {
    let reduced_motion = use_prefers_reduced_motion();

    // determine initial animation state on component launch
    let animation_state = RwSignal::new({
        let state = animation
            .get_untracked()
            .map(|(start, _)| IntroAnimationState::from(start))
            .unwrap_or_default();

        keyframes![(state, 0.0)]
    });

    // state to track keyframes for reduced motion animation

    let reduced_motion_state = RwSignal::new(ReducedMotionState {
        total_keyframes: animation_state.get_untracked().keyframes(),
        current_keyframe: 0,
        accumulated_time: 0.0,
    });

    let completed = RwSignal::new(false);

    let Pausable { pause, resume, .. } = use_raf_fn_with_options(
        move |UseRafFnCallbackArgs { delta, .. }| {
            let mut state = animation_state();
            let remainder = if reduced_motion() {
                let mut rm_state = reduced_motion_state();
                rm_state.accumulated_time += delta;

                // Get the current keyframe pair (current, next)
                let (_, next_kf) = state.pair();

                if let Some(next_kf) = next_kf {
                    // If we've accumulated enough time to reach the next keyframe
                    if rm_state.accumulated_time >= next_kf.time() {
                        // Jump directly to the next keyframe time
                        let remainder = state.advance_to(next_kf.time());
                        rm_state.current_keyframe += 1;
                        animation_state.set(state);
                        reduced_motion_state.set(rm_state);
                        Some(remainder)
                    } else {
                        // Not enough time accumulated yet, just update state
                        reduced_motion_state.set(rm_state);
                        None
                    }
                } else {
                    // No next keyframe (animation finished)
                    reduced_motion_state.set(rm_state);
                    None
                }
            } else {
                let rem = state.duration() - state.time();
                let remainder = state.advance_by(delta.min(rem));
                animation_state.set(state);
                Some(remainder)
            };

            if remainder.is_some_and(|r| r <= 0.0) {
                completed.set(true);
                on_animation_ended.iter().for_each(|cb| cb.run(()));
            }
        },
        UseRafFnOptions::default().immediate(false),
    );

    // play and pause animations
    // TODO: build correct animation end states
    Effect::new({
        let pause = pause.clone();

        move |_| {
            if animation().is_some() {
                resume();
                // TODO: actual number of keyframes
                reduced_motion_state.set(ReducedMotionState {
                    total_keyframes: animation_state.get_untracked().keyframes(),
                    current_keyframe: 0,
                    accumulated_time: 0.0,
                });
            } else {
                pause();
            }
        }
    });

    // stop raf once sequence ended
    Effect::new(move |_| {
        if completed() {
            pause();
        }
    });

    let now_state = move || animation_state().now();

    let view_box_value = move || {
        let (start, end) = now_state().view_box;
        format!("{} {} {} {}", start.x, start.y, end.x, end.y)
    };

    view! {
        <>
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
                            format!("M{} {}L{} {}", pos_start.x, pos_start.y, pos_end.x, pos_end.y)
                        } />
                        <path d=move || {
                            let (pos_start, pos_end) = now_state().string_2_position;
                            format!("M{} {}L{} {}", pos_start.x, pos_start.y, pos_end.x, pos_end.y)
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
                                        // Draw whole circle when split is zero
                                        view! { <circle r=radius cx=pos.x cy=pos.y /> }
                                            .into_any()
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
        </>
    }
}
