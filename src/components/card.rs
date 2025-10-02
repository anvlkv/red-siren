mod animation;
mod types;

use animation::{
    adapt_appear, adapt_enter, adapt_leave, CARD_APPEAR_PERSPECTIVE_CM, CARD_MAX_BLUR,
    CARD_PERSPECTIVE_CM,
};
use types::CardEffects;
pub use types::{CardAnimation, StretchAxis};

use std::time::Duration;

use keyframe::{
    functions::{EaseInCubic, EaseOutCubic},
    keyframes, AnimationSequence,
};
use leptos::prelude::*;
use leptos_use::use_prefers_reduced_motion;

use crate::{
    components::{UiPadding, UiVariant},
    util::raf_fn_fps::use_raf_fn_with_fps_signal,
};

const THICKNESS_PX: f64 = 8.0;

/// Generic Card component (MAYA DRY KISS).
///
/// Features:
/// - Variants: Elevated | Outline | Ghost
/// - Padding: None | Sm | Md | Lg
/// - Rounded corners by default (can toggle)
/// - Optional interactive state (hover/active transitions; on_click)
/// - Imperative rotateY animations controlled by parent via tx props
/// - Full-width toggle
///
/// Example:
/// view! {
///   <Card
///     start_animation=start_animation
///     on_animation_done=Callback::new(|(tx, kind)| {
///         // Optionally branch on kind: EnterY | LeaveY | Appear
///     })
///   >
///     <p>"Hello card"</p>
///   </Card>
/// }
#[component]
pub fn Card(
    // Main content
    children: Children,

    // Appearance
    #[prop(optional, into)] variant: UiVariant,
    #[prop(optional, into)] padding: UiPadding,
    #[prop(optional, into)] rounded: bool,
    #[prop(optional, into)] full_width: bool,
    #[prop(optional, into)] class: Signal<String>,

    // Behavior
    #[prop(optional)] interactive: bool,

    // Unified animation interface:
    // Provide (tx_id, CardAnimation) to start an animation; done callback notified when sequence completes.
    #[prop(optional)] start_animation: Signal<Option<CardAnimation>>,
    #[prop(optional)] on_animation_done: Option<Callback<()>>,
    // Imperative animation controls (optional):
    // Set a new tx_id to start an animation. Card will run animations and then call on_*_done if provided.

    // Animation tuning (rotateY)

    // Animation tuning (appear)
) -> impl IntoView {
    // Base layout and typography colors tuned to the existing theme
    let base = "text-black dark:text-red \
                transition-all duration-200 \
                focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 transform-3d will-change-transform relative";

    let bg_and_border = match variant {
        UiVariant::Solid => {
            "\
            bg-red dark:bg-black \
            shadow-xl shadow-gray dark:shadow-cinnabar"
        }
        UiVariant::Outline => {
            "\
            bg-transparent border-2 border-black dark:border-red \
            hover:shadow-sm hover:shadow-gray dark:hover:shadow-cinnabar"
        }
        UiVariant::Ghost => {
            "\
            bg-transparent"
        }
    };

    let rounding = if rounded { "rounded-xl" } else { "rounded-lg" };

    let padding_cls = padding.tw_class();

    let width_cls = if full_width { "w-full" } else { "w-max" };

    let interactive_cls = if interactive {
        "\
        cursor-pointer \
        hover:scale-[1.01] \
        active:scale-[0.995]"
    } else {
        "cursor-default"
    };

    let class = Signal::derive(move || {
        format!(
            "{base} {bg_and_border} {rounding} {padding_cls} {width_cls} {interactive_cls} {}",
            class()
        )
    });

    // Built-in animation plumbing (imperative)
    let reduced_motion = use_prefers_reduced_motion();

    let anim_ms_sig = RwSignal::new(0.0_f64);
    let transform_seq = RwSignal::new(keyframes![(
        CardEffects {
            x_px: 0.0,
            y_px: 0.0,
            z_px: 0.0,
            tilt_x_deg: 0.0,
            rot_y_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            perspective: 0.0,
            blur: 0.0
        },
        0.0
    )]);

    // RAF loop (FPS throttled via reactive signal)
    let fps_signal = Signal::derive(move || {
        if reduced_motion() {
            animation::REDUCED_FPS
        } else {
            animation::NORMAL_FPS
        }
    });

    let pausable = use_raf_fn_with_fps_signal(
        {
            move |args: leptos_use::UseRafFnCallbackArgs| {
                let reduced = reduced_motion();
                if reduced {
                    // In reduced motion, fast-forward to end (no intermediate frames)
                    transform_seq.update(|seq| {
                        seq.advance_to(anim_ms_sig());
                    });
                } else {
                    transform_seq.update(|seq| {
                        let rem = seq.duration() - seq.time();
                        seq.advance_by(args.delta.min(rem));
                    });
                }
            }
        },
        Signal::derive(move || fps_signal.get()),
    );
    let anim_pause = pausable.pause;
    let anim_resume = pausable.resume;
    let anim_active = pausable.is_active;

    // Start animation when parent sets a new (tx_id, CardAnimation)
    Effect::new(move |_| {
        if let Some(animation) = start_animation() {
            // FPS already chosen (normal vs reduced only)
            match animation {
                CardAnimation::Appear3D {
                    from_x_px,
                    from_y_px,
                    from_z_px,
                    from_tilt_x_deg,
                    stretch_axis,
                    stretch_factor,
                } => {
                    // Adapt (duration, depth, stretch, tilt) using helper
                    let adapt = adapt_appear(
                        from_x_px,
                        from_y_px,
                        from_z_px,
                        from_tilt_x_deg,
                        stretch_factor,
                    );
                    anim_ms_sig.set(animation::CARD_APPEAR_BASE_MS);
                    let edge_ratio = adapt.edge_ratio;
                    let final_stretch = adapt.final_stretch_factor;
                    let adj_tilt = adapt.adjusted_tilt_deg;
                    let adj_from_z = adapt.adjusted_from_z;
                    let curve_point1_t = adapt.curve_point1_t;
                    let curve_point2_t = adapt.curve_point2_t;
                    let total_t = adapt.total_t;
                    let (sx_stretch, sy_stretch) = match stretch_axis {
                        StretchAxis::X => (final_stretch, 1.0),
                        StretchAxis::Y => (1.0, final_stretch),
                    };

                    // Cubic curve: start deep → curve point 1 → curve point 2 → center
                    transform_seq.set(keyframes![
                        (
                            CardEffects {
                                x_px: from_x_px,
                                y_px: from_y_px,
                                z_px: adj_from_z,
                                tilt_x_deg: adj_tilt,
                                rot_y_deg: 0.0,
                                scale_x: sx_stretch,
                                scale_y: sy_stretch,
                                perspective: CARD_APPEAR_PERSPECTIVE_CM * adapt.perspective_scale,
                                blur: CARD_MAX_BLUR,
                            },
                            0.0
                        ),
                        (
                            CardEffects {
                                x_px: from_x_px * 0.33,
                                y_px: from_y_px * 0.33,
                                z_px: adj_from_z * 0.33,
                                tilt_x_deg: adj_tilt * 0.33,
                                rot_y_deg: 0.0,
                                scale_x: 1.0 + (sx_stretch - 1.0) * 0.67,
                                scale_y: 1.0 + (sy_stretch - 1.0) * 0.67,
                                perspective: CARD_APPEAR_PERSPECTIVE_CM * adapt.perspective_scale,
                                blur: CARD_MAX_BLUR * 0.67,
                            },
                            curve_point1_t,
                            EaseOutCubic
                        ),
                        (
                            CardEffects {
                                x_px: from_x_px * 0.1,
                                y_px: from_y_px * 0.1,
                                z_px: adj_from_z * 0.1,
                                tilt_x_deg: adj_tilt * 0.1,
                                rot_y_deg: 0.0,
                                scale_x: 1.0 + (sx_stretch - 1.0) * 0.33,
                                scale_y: 1.0 + (sy_stretch - 1.0) * 0.33,
                                perspective: CARD_PERSPECTIVE_CM * adapt.perspective_scale,
                                blur: CARD_MAX_BLUR * 0.33,
                            },
                            curve_point2_t,
                            EaseOutCubic
                        ),
                        (
                            CardEffects {
                                x_px: 0.0,
                                y_px: 0.0,
                                z_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: 0.0,
                                scale_x: 1.0,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: 0.0,
                            },
                            total_t,
                            EaseOutCubic
                        )
                    ]);
                    anim_resume();
                    log::debug!("Card: APPEAR3D edge_ratio={edge_ratio:.2} ms={} tilt_in={from_tilt_x_deg} tilt_adj={adj_tilt} stretch_final={final_stretch}", animation::CARD_APPEAR_BASE_MS);
                }
                CardAnimation::EnterTravel3D {
                    from_x_px,
                    from_z_px,
                    from_rot_y_deg,
                    to_rot_y_deg,
                } => {
                    let enter = adapt_enter(from_x_px, from_z_px, from_rot_y_deg);
                    anim_ms_sig.set(animation::CARD_ENTER_BASE_MS);
                    let edge_ratio = enter.edge_ratio;
                    let adj_from_rot = enter.adjusted_from_rot_y_deg;
                    let adj_from_z = enter.adjusted_from_z;
                    let curve_point1_t = enter.curve_point1_t;
                    let curve_point2_t = enter.curve_point2_t;
                    let total_t = enter.total_t;
                    let stretch_start = enter.entry_stretch_start;

                    // Cubic curve: back-left → curve point 1 → curve point 2 → center
                    // With rotation following curve and stretch effects
                    transform_seq.set(keyframes![
                        (
                            CardEffects {
                                x_px: from_x_px,
                                y_px: 0.0,
                                z_px: adj_from_z,
                                tilt_x_deg: 0.0,
                                rot_y_deg: adj_from_rot,
                                scale_x: stretch_start, // Stretched at start
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM * enter.perspective_scale,
                                blur: CARD_MAX_BLUR,
                            },
                            0.0
                        ),
                        (
                            CardEffects {
                                x_px: from_x_px * 0.33,
                                y_px: 0.0,
                                z_px: adj_from_z * 0.33,
                                tilt_x_deg: 0.0,
                                rot_y_deg: adj_from_rot * 0.67,
                                scale_x: 1.0 + (stretch_start - 1.0) * 0.67, // Gradually return to normal
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM * enter.perspective_scale,
                                blur: CARD_MAX_BLUR * 0.67,
                            },
                            curve_point1_t,
                            EaseOutCubic
                        ),
                        (
                            CardEffects {
                                x_px: from_x_px * 0.1,
                                y_px: 0.0,
                                z_px: adj_from_z * 0.1,
                                tilt_x_deg: 0.0,
                                rot_y_deg: adj_from_rot * 0.33,
                                scale_x: 1.0 + (stretch_start - 1.0) * 0.33, // Nearly normal
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: CARD_MAX_BLUR * 0.33,
                            },
                            curve_point2_t,
                            EaseOutCubic
                        ),
                        (
                            CardEffects {
                                x_px: 0.0,
                                y_px: 0.0,
                                z_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: to_rot_y_deg,
                                scale_x: 1.0, // Normal scale at end
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: 0.0,
                            },
                            total_t,
                            EaseOutCubic
                        )
                    ]);
                    anim_resume();
                    log::debug!("Card: ENTER_TRAVEL3D edge_ratio={edge_ratio:.2} ms={} rot_adj={adj_from_rot}->{to_rot_y_deg}", animation::CARD_ENTER_BASE_MS);
                }
                CardAnimation::LeaveTravel3D {
                    to_x_px,
                    to_z_px,
                    to_rot_y_deg,
                } => {
                    let leave = adapt_leave(to_x_px, to_z_px, to_rot_y_deg);
                    anim_ms_sig.set(animation::CARD_LEAVE_BASE_MS);
                    let edge_ratio = leave.edge_ratio;
                    let adj_to_rot = leave.adjusted_to_rot_y_deg;
                    let adj_to_z = leave.adjusted_to_z;
                    let curve_point1_t = leave.curve_point1_t;
                    let curve_point2_t = leave.curve_point2_t;
                    let total_t = leave.total_t;
                    let stretch_end = leave.leave_stretch_end;

                    // Cubic curve: center → curve point 1 → curve point 2 → back-right
                    // With rotation following curve and dramatic stretch at exit
                    transform_seq.set(keyframes![
                        (
                            CardEffects {
                                x_px: 0.0,
                                y_px: 0.0,
                                z_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: 0.0,
                                scale_x: 1.0, // Start normal
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: 0.0,
                            },
                            0.0
                        ),
                        (
                            CardEffects {
                                x_px: to_x_px * 0.33,
                                y_px: 0.0,
                                z_px: adj_to_z * 0.33,
                                tilt_x_deg: 0.0,
                                rot_y_deg: adj_to_rot * 0.33,
                                scale_x: 1.0 + (stretch_end - 1.0) * 0.33, // Begin stretching
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM * leave.perspective_scale,
                                blur: CARD_MAX_BLUR * 0.33,
                            },
                            curve_point1_t,
                            EaseInCubic
                        ),
                        (
                            CardEffects {
                                x_px: to_x_px * 0.67,
                                y_px: 0.0,
                                z_px: adj_to_z * 0.67,
                                tilt_x_deg: 0.0,
                                rot_y_deg: adj_to_rot * 0.67,
                                scale_x: 1.0 + (stretch_end - 1.0) * 0.67, // More stretching
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: CARD_MAX_BLUR * 0.67,
                            },
                            curve_point2_t,
                            EaseInCubic
                        ),
                        (
                            CardEffects {
                                x_px: to_x_px,
                                y_px: 0.0,
                                z_px: adj_to_z,
                                tilt_x_deg: 0.0,
                                rot_y_deg: adj_to_rot,
                                scale_x: stretch_end, // Maximum stretch at exit
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: CARD_MAX_BLUR,
                            },
                            total_t,
                            EaseInCubic
                        )
                    ]);
                    anim_resume();
                    log::debug!("Card: LEAVE_TRAVEL3D edge_ratio={edge_ratio:.2} ms={} rot_adj={adj_to_rot}", animation::CARD_LEAVE_BASE_MS);
                }
            }
        }
    });

    let transform_style = move || {
        let s = transform_seq().now();
        format!(
            "translate3d({}px, {}px, {}px) rotateX({}deg) rotateY({}deg) scale({},{})",
            s.x_px, s.y_px, s.z_px, s.tilt_x_deg, s.rot_y_deg, s.scale_x, s.scale_y
        )
    };

    let perspective = move || {
        let s = transform_seq().now();
        format!("perspective({}cm)", s.perspective)
    };

    let blur = move || {
        let s = transform_seq().now();
        format!("blur({}px)", s.blur)
    };

    let thick_edge_class =
        move |pos: &str| format!("absolute {pos} bg-cinnabar/25 dark:bg-gray/25 blur-xs ");

    // Completion notifications
    Effect::new(move |_| {
        if transform_seq().finished() && anim_active() {
            anim_pause();
            if let Some(cb) = on_animation_done {
                set_timeout(move || cb.run(()), Duration::from_millis(20));
            }
        }
    });

    view! {
        <div
            style:transform=perspective
            style:filter=blur
            role=if interactive { "button" } else { "group" }
            tabindex=if interactive { Some("0") } else { None }
        >
            <div class=class style:transform=transform_style style:transform-origin="50% 100%">
                <div
                    class="relative backface-hidden"
                    style:transform=format!("translateZ({THICKNESS_PX}px)")
                >
                    <div class="card-body contents">{children()}</div>
                </div>

                <div
                    class=move || thick_edge_class("inset-x-0 top-0 h-2")
                    style:transform=format!("rotateX(90deg) translateZ({}px)", THICKNESS_PX / 2.0)
                    aria-hidden="true"
                    role="presentation"
                />
                <div
                    class=move || thick_edge_class("inset-x-0 bottom-0 h-2")
                    style:transform=format!("rotateX(-90deg) translateZ({}px)", THICKNESS_PX / 2.0)
                    aria-hidden="true"
                    role="presentation"
                />
                <div
                    class=move || thick_edge_class("inset-y-0 left-0 w-2")
                    style:transform=format!("rotateY(-90deg) translateZ({}px)", THICKNESS_PX / 2.0)
                    aria-hidden="true"
                    role="presentation"
                />
                <div
                    class=move || thick_edge_class("inset-y-0 right-0 w-2")
                    style:transform=format!("rotateY(90deg) translateZ({}px)", THICKNESS_PX / 2.0)
                    aria-hidden="true"
                    role="presentation"
                />
            </div>
        </div>
    }
}
