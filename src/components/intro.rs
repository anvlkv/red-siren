use keyframe::{keyframes, AnimationSequence, CanTween};
use leptos::prelude::*;
use leptos_use::{
    use_prefers_reduced_motion, use_raf_fn_with_options, utils::Pausable, UseRafFnCallbackArgs,
    UseRafFnOptions,
};
use mint::{Point2, Vector2};
use shared::orientation::LayoutOrientation;

use crate::{
    components::Wavering,
    util::animation::{tween_tuple_vectors, tween_vectors, ReducedMotionState},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroAnimationTarget {
    Intro,
    Tuner,
    Instrument(shared::instrument::Layout),
}

#[derive(Debug, Clone, PartialEq)]
struct IntroAnimationState {
    view_box: (Point2<f32>, Point2<f32>),
    picture_opacity: f32,
    suns_positions: Vec<Point2<f32>>,
    suns_splits: Vec<Vector2<f32>>,
    sun_radius: f32,
    keybands_positions: Vec<(Point2<f32>, Point2<f32>)>,
    string_1_position: (Point2<f32>, Point2<f32>),
    string_2_position: (Point2<f32>, Point2<f32>),
    strings_rotation: (f32, Point2<f32>),
    strings_stroke: f32,
}

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

#[component]
fn SplitSuns(
    pos: Point2<f32>,
    radius: f32,
    split: Vector2<f32>,
    stroke_width: f32,
) -> impl IntoView {
    // Calculate points for the two semi-circles
    let center_x = pos.x;
    let center_y = pos.y;
    let split_x = split.x;
    let split_y = split.y;

    // Calculate the angle of the split vector
    let angle = split_y.atan2(split_x);
    let start_angle_1 = angle;
    let end_angle_1 = angle + std::f32::consts::PI;
    let start_angle_2 = angle + std::f32::consts::PI;
    let end_angle_2 = angle + 2.0 * std::f32::consts::PI;

    // Generate arc paths for semi-circles
    let arc_1 = generate_arc_path(center_x, center_y, radius, start_angle_1, end_angle_1);
    let arc_2 = generate_arc_path(center_x, center_y, radius, start_angle_2, end_angle_2);

    // Calculate connection line endpoints
    let line_start_x = center_x + radius * start_angle_1.cos();
    let line_start_y = center_y + radius * start_angle_1.sin();
    let line_end_x = center_x + radius * end_angle_1.cos();
    let line_end_y = center_y + radius * end_angle_1.sin();

    view! {
        <>
            <path d=arc_1 />
            <path d=arc_2 />
            <path
                d=format!("M{} {}L{} {}", line_start_x, line_start_y, line_end_x, line_end_y)
                stroke-width=stroke_width
            />
        </>
    }
}

fn generate_arc_path(cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32) -> String {
    let start_x = cx + r * start_angle.cos();
    let start_y = cy + r * start_angle.sin();
    let end_x = cx + r * end_angle.cos();
    let end_y = cy + r * end_angle.sin();

    let large_arc_flag = if (end_angle - start_angle).abs() > std::f32::consts::PI {
        1
    } else {
        0
    };
    let sweep_flag = if end_angle > start_angle { 1 } else { 0 };

    format!(
        "M{} {}A{} {} 0 {} {} {} {}",
        start_x, start_y, r, r, large_arc_flag, sweep_flag, end_x, end_y
    )
}

impl Default for IntroAnimationState {
    fn default() -> Self {
        Self {
            view_box: (Point2 { x: 0.0, y: 0.0 }, Point2 { x: 430.0, y: 932.0 }),
            picture_opacity: 1.0,
            suns_positions: vec![Point2 { x: 107.0, y: 164.0 }],
            suns_splits: vec![Vector2 { x: 0.0, y: 0.0 }],
            sun_radius: 39.0,
            keybands_positions: Vec::default(),
            string_1_position: (
                Point2 {
                    x: 73.7113,
                    y: 576.054,
                },
                Point2 {
                    x: 73.7113 + 53.653,
                    y: 576.054,
                },
            ),
            string_2_position: (
                Point2 {
                    x: 73.7113,
                    y: 576.054 + 8.25253,
                },
                Point2 {
                    x: 73.7113 + 53.653,
                    y: 576.054 + 8.25253,
                },
            ),
            strings_rotation: (
                -17.1246,
                Point2 {
                    x: 48.3365,
                    y: 585.964,
                },
            ),
            strings_stroke: 2.0,
        }
    }
}

impl From<IntroAnimationTarget> for IntroAnimationState {
    fn from(value: IntroAnimationTarget) -> Self {
        match value {
            IntroAnimationTarget::Intro => Self::default(),
            IntroAnimationTarget::Tuner => todo!(),
            IntroAnimationTarget::Instrument(layout) => {
                let total_keys =
                    layout.num_groups.get() as usize * layout.num_keys_per_group.get() as usize;
                let num_groups = layout.num_groups.get() as usize;
                let keys_per_group = layout.num_keys_per_group.get() as usize;

                // Calculate sun positions (keys) and keyband positions
                let mut suns_positions = Vec::with_capacity(total_keys);
                let mut keybands_positions = Vec::with_capacity(total_keys);

                // Determine main and cross axes based on orientation
                let (main_axis, cross_axis) = match layout.orientation {
                    LayoutOrientation::Horizontal => (layout.space.x, layout.space.y),
                    LayoutOrientation::Vertical => (layout.space.y, layout.space.x),
                };

                // Calculate available space after safe area padding
                let available_main =
                    main_axis - layout.safe_area_padding[0] - layout.safe_area_padding[2];
                let available_cross =
                    cross_axis - layout.safe_area_padding[1] - layout.safe_area_padding[3];

                // Calculate group spacing
                let total_groups_gap = layout.groups_gap * (num_groups - 1) as f32;
                let group_main_size = (available_main - total_groups_gap) / num_groups as f32;

                // Calculate key spacing within groups
                let total_key_gaps = layout.key_bands_gap * (keys_per_group - 1) as f32;
                let key_cross_size = (available_cross - total_key_gaps) / keys_per_group as f32;

                for group_idx in 0..num_groups {
                    let group_main_offset = layout.safe_area_padding[0]
                        + (group_main_size + layout.groups_gap) * group_idx as f32;

                    for key_idx in 0..keys_per_group {
                        let key_cross_offset = layout.safe_area_padding[1]
                            + (key_cross_size + layout.key_bands_gap) * key_idx as f32;

                        // Calculate position based on orientation
                        let (x, y) = match layout.orientation {
                            LayoutOrientation::Horizontal => (
                                group_main_offset + group_main_size * 0.5,
                                key_cross_offset + key_cross_size * 0.5,
                            ),
                            LayoutOrientation::Vertical => (
                                key_cross_offset + key_cross_size * 0.5,
                                group_main_offset + group_main_size * 0.5,
                            ),
                        };

                        suns_positions.push(Point2 { x, y });

                        // Calculate keyband positions (full-rounded rectangles around keys)
                        let band_start = Point2 {
                            x: x - layout.key_band_length * 0.5,
                            y: y - layout.key_band_breadth * 0.5,
                        };
                        let band_end = Point2 {
                            x: x + layout.key_band_length * 0.5,
                            y: y + layout.key_band_breadth * 0.5,
                        };
                        keybands_positions.push((band_start, band_end));
                    }
                }

                Self {
                    view_box: (
                        Point2 { x: 0.0, y: 0.0 },
                        Point2 {
                            x: layout.space.x,
                            y: layout.space.y,
                        },
                    ),
                    picture_opacity: 0.0,
                    suns_positions,
                    suns_splits: (0..total_keys)
                        .map(|_| Vector2 { x: 0.0, y: 0.0 })
                        .collect(), // splits not used
                    sun_radius: layout.key_radius,
                    keybands_positions,
                    string_1_position: layout.left_string_position,
                    string_2_position: layout.right_string_position,
                    strings_rotation: (0.0, Point2 { x: 0.0, y: 0.0 }), // rotation not specified, use default
                    strings_stroke: 1.0,
                }
            }
        }
    }
}

impl CanTween for IntroAnimationState {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let suns_positions = tween_vectors(&from.suns_positions, &to.suns_positions, time);
        let suns_splits = tween_vectors(&from.suns_splits, &to.suns_splits, time);
        let keybands_positions =
            tween_tuple_vectors(&from.keybands_positions, &to.keybands_positions, time);

        Self {
            view_box: (
                CanTween::ease(from.view_box.0, to.view_box.0, time),
                CanTween::ease(from.view_box.1, to.view_box.1, time),
            ),
            suns_positions,
            suns_splits,
            keybands_positions,
            sun_radius: CanTween::ease(from.sun_radius, to.sun_radius, time),
            picture_opacity: CanTween::ease(from.picture_opacity, to.picture_opacity, time),
            string_1_position: (
                CanTween::ease(from.string_1_position.0, to.string_1_position.0, time),
                CanTween::ease(from.string_1_position.1, to.string_1_position.1, time),
            ),
            string_2_position: (
                CanTween::ease(from.string_2_position.0, to.string_2_position.0, time),
                CanTween::ease(from.string_2_position.1, to.string_2_position.1, time),
            ),
            strings_rotation: (
                CanTween::ease(from.strings_rotation.0, to.strings_rotation.0, time),
                CanTween::ease(from.strings_rotation.1, to.strings_rotation.1, time),
            ),
            strings_stroke: CanTween::ease(from.strings_stroke, to.strings_stroke, time),
        }
    }
}

const STONE_PATH: &str = "M350.5 815.501C388.533 848.848 429 933.001 429 933.001H85C85 933.001 265.246 740.749 350.5 815.501Z";

const SIREN_ARM_PATH: &str = "M203.1 547.176C202.815 546.205 202.573 544.98 202.502 543.508L202.517 543.279L202.517 543.279L202.521 543.217C202.763 539.584 202.986 536.249 202.905 534.005C202.885 533.435 202.844 532.914 202.775 532.463C202.708 532.025 202.604 531.591 202.427 531.23C202.255 530.88 201.915 530.412 201.296 530.295C200.704 530.182 200.207 530.461 199.886 530.711C199.384 531.101 198.969 531.634 198.623 532.166C198.272 532.704 197.956 533.301 197.672 533.859C197.58 534.038 197.493 534.211 197.41 534.378C197.223 534.749 197.052 535.09 196.881 535.403C196.626 535.869 196.433 536.153 196.293 536.293C196.018 536.568 195.86 536.635 195.813 536.65C195.804 536.646 195.792 536.638 195.777 536.627C195.678 536.554 195.533 536.397 195.337 536.099C195.149 535.814 194.962 535.474 194.743 535.076L194.733 535.057L194.709 535.014C194.298 534.268 193.759 533.29 193.014 532.629C192.614 532.274 192.12 531.976 191.518 531.855C190.911 531.734 190.277 531.81 189.628 532.07C189.146 532.263 188.76 532.565 188.489 532.971C188.225 533.365 188.111 533.797 188.068 534.197C187.99 534.925 188.138 535.741 188.254 536.379C188.258 536.402 188.262 536.424 188.266 536.447C188.399 537.183 188.487 537.715 188.443 538.117C188.424 538.295 188.383 538.382 188.354 538.425C188.333 538.456 188.287 538.511 188.145 538.565C187.476 538.818 187.191 538.692 187.022 538.575C186.757 538.391 186.514 538.029 186.207 537.435C186.161 537.347 186.112 537.248 186.059 537.145C185.961 536.949 185.854 536.737 185.754 536.554C185.594 536.261 185.397 535.934 185.147 535.653C184.891 535.367 184.541 535.084 184.068 534.955C183.586 534.823 183.096 534.884 182.629 535.071C181.662 535.458 180.808 535.98 180.6 536.885C180.496 537.334 180.591 537.734 180.703 538.027C180.807 538.298 180.957 538.558 181.067 538.748C181.071 538.755 181.075 538.762 181.079 538.769C181.343 539.225 181.461 539.465 181.469 539.7C181.474 539.852 181.435 540.192 180.818 540.768C179.997 541.534 179.697 541.422 179.649 541.404C179.648 541.404 179.647 541.403 179.646 541.403C179.562 541.373 179.432 541.294 179.24 541.1C179.046 540.904 178.851 540.656 178.605 540.339C178.586 540.316 178.568 540.292 178.549 540.267C178.108 539.699 177.491 538.903 176.63 538.45C175.62 537.918 174.434 537.914 173.053 538.605C172.687 538.788 172.197 539.161 172.146 539.835C172.105 540.391 172.407 540.836 172.595 541.081C172.814 541.365 173.111 541.661 173.418 541.952C173.58 542.106 173.772 542.284 173.974 542.47C174.145 542.628 174.322 542.792 174.493 542.953C176.065 544.428 178.162 546.582 179.581 549.894L179.604 549.947L179.633 549.998C181.77 553.714 183.7 558.524 185.453 562.909L185.501 563.029C186.354 565.163 187.166 567.194 187.93 568.911C188.7 570.64 189.459 572.136 190.207 573.108C190.607 573.628 191.23 574.168 191.977 574.714C192.738 575.271 193.686 575.875 194.786 576.518C196.986 577.803 199.844 579.267 203.142 580.832C209.74 583.962 218.153 587.518 226.698 590.867C235.243 594.216 243.934 597.364 251.091 599.677C254.668 600.833 257.872 601.783 260.488 602.446C263.071 603.1 265.174 603.499 266.5 603.499C267.78 603.499 269.921 603.235 272.647 602.78C275.396 602.322 278.804 601.658 282.65 600.836C290.345 599.191 299.823 596.906 309.354 594.339C318.883 591.773 328.479 588.922 336.405 586.146C340.367 584.758 343.924 583.384 346.852 582.069C349.763 580.763 352.116 579.487 353.625 578.28C357.518 575.165 361.117 570.227 362.601 564.932C364.096 559.597 363.455 553.807 358.689 549.274C353.194 544.047 344.57 543.503 335.1 545.321C325.575 547.15 314.874 551.428 304.875 556.287C294.865 561.151 285.506 566.623 278.652 570.876C275.225 573.003 272.421 574.827 270.473 576.12C269.499 576.766 268.739 577.28 268.222 577.633C268.067 577.739 267.933 577.83 267.823 577.906L207.101 560.277L208.242 551.376C208.354 550.504 208.6 549.538 208.893 548.53C209.005 548.145 209.127 547.742 209.25 547.338C209.436 546.727 209.622 546.115 209.773 545.559C210.026 544.63 210.235 543.676 210.217 542.859C210.198 542.026 209.923 541.06 208.923 540.593C208.256 540.281 207.571 540.23 206.921 540.449C206.301 540.657 205.805 541.079 205.415 541.545C204.647 542.462 204.089 543.793 203.682 545.053C203.451 545.769 203.258 546.498 203.1 547.176Z";

const SIREN_PATH_1: &str = "M240 585.001C250 574.001 257.201 559.354 245.5 549.501C236 541.501 171 618.501 171 618.501L110.126 585.001C110.126 585.001 125.204 566.227 117 571.501C110 576.001 112.5 568.501 110.126 565.001C109.536 564.131 109.5 561.07 107.5 559.5C105.5 557.93 107.5 568.5 104.231 571.001C100.962 573.503 95.0002 557.5 93.0001 559C91 560.501 96.9999 574.5 95.0001 574.001C93.0002 573.502 87.5 561 87.5 565C87.5 569 88.3494 571.894 88.0001 572.501C87.4631 573.433 86.7409 571.227 82.5 568.5C75.4999 564 102.231 594.74 102.231 594.74C102.231 594.74 158.5 642.001 171 641.001C183.5 640.001 216.673 610.661 240 585.001Z";

const SIREN_PATH_2: &str = "M299 412.501L289 429.5L272.5 485.5C272.5 485.5 304.536 532.826 285.5 546.5C275.132 553.947 257.649 550.718 245 549C231.128 547.116 220.068 546.44 212 535C199.128 516.746 226.224 503.117 233.5 482C242.142 456.919 236.638 438.473 251.5 416.5C258.16 406.654 260.726 403.637 272.5 402.001C284.411 400.345 299 412.501 299 412.501Z";

const SIREN_PATH_3: &str = "M234 609C229.5 593 245 549.5 245 549.5L278.5 545.5C278.5 545.5 282.487 522.685 278.5 509C275.593 499.02 266 485.5 266 485.5L289 429.5C289 429.5 314.485 467.233 319.5 495C323.005 514.408 319.5 545.5 319.5 545.5L358.5 549.5C358.5 549.5 355 631 358.5 687C362 743 370.934 745.097 378 782.5C388.874 840.066 294 932.5 294 932.5H165C165 932.5 175.661 837.276 198.5 782.5C214.988 742.955 249.079 729.708 252.5 687C254.999 655.794 238.5 625 234 609Z";

const SIREN_FRONT_PATH_1: &str ="M299.5 411.5L289 430C289 430 313.862 470.508 319 499.5C323.331 523.937 294.949 552.806 318 562C336.745 569.478 359.087 557.193 363.5 537.501C366.814 522.711 353.312 516.031 349 501.5C342.323 478.998 352.064 462.426 340.5 442C333.957 430.443 329.828 417.54 318 411.5C308.5 406.648 299.5 411.5 299.5 411.5Z";
const SIREN_FRONT_PATH_2: &str ="M83.5312 578.5C82.0312 579 80.0313 568 81.5312 568C83.1881 568 83.2888 569.019 84.0312 570.5C84.4 571.236 84.5918 578.146 83.5312 578.5Z";
const SIREN_FRONT_PATH_3: &str ="M89.2847 574.983C87.9308 575.506 86.1256 564 87.4795 564C88.975 564 89.0659 565.066 89.7361 566.615C90.0689 567.384 90.2421 574.613 89.2847 574.983Z";
const SIREN_FRONT_PATH_4: &str ="M95.0463 573.976C93.2411 574.69 90.8342 559 92.6393 559C94.6333 559 94.7546 560.453 95.6481 562.566C96.0919 563.615 96.3228 573.472 95.0463 573.976Z";
const SIREN_FRONT_PATH_5: &str ="M100.8 570.575C99.3555 570.215 106.417 558.455 107.476 559.299C108.645 560.231 107.932 561.272 107.315 563.122C107.009 564.041 101.821 570.83 100.8 570.575Z";
