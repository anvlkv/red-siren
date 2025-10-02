mod animation;
mod types;

use animation::{adapt_enter, adapt_leave, CARD_MAX_BLUR, CARD_PERSPECTIVE_CM};
use types::CardEffects;
pub use types::{CardAnimation, EdgeSide};

use std::time::Duration;

use keyframe::{
    functions::{EaseIn, EaseOut},
    keyframes, AnimationSequence,
};
use leptos::prelude::*;
use leptos_use::use_prefers_reduced_motion;
use leptos_use::use_raf_fn;

use crate::components::{UiPadding, UiVariant};

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
    let (current_edge_side, set_current_edge_side) = signal(None::<EdgeSide>);
    let transform_seq = RwSignal::new(keyframes![(
        CardEffects {
            x_px: 0.0,
            y_px: 0.0,
            z_px: 0.0,
            rot_x_deg: 0.0,
            rot_y_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            perspective: 0.0,
            blur: 0.0
        },
        0.0
    )]);

    let pausable = use_raf_fn({
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
    });
    let anim_pause = pausable.pause;
    let anim_resume = pausable.resume;
    let anim_active = pausable.is_active;

    // Start animation when parent sets a new (tx_id, CardAnimation)
    Effect::new(move |_| {
        if let Some(animation) = start_animation() {
            match animation {
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
                    let stretch_start = enter.entry_stretch_start;

                    transform_seq.set(keyframes![
                        (
                            CardEffects {
                                x_px: from_x_px,
                                y_px: 0.0,
                                z_px: adj_from_z,
                                rot_x_deg: 0.0,
                                rot_y_deg: adj_from_rot,
                                scale_x: stretch_start,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM * enter.perspective_scale,
                                blur: CARD_MAX_BLUR,
                            },
                            0.0
                        ),
                        (
                            CardEffects {
                                x_px: 0.0,
                                y_px: 0.0,
                                z_px: 0.0,
                                rot_x_deg: 0.0,
                                rot_y_deg: to_rot_y_deg,
                                scale_x: 1.0,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: 0.0,
                            },
                            animation::CARD_ENTER_BASE_MS,
                            EaseOut
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
                    let stretch_end = leave.leave_stretch_end;

                    // Simple 2-keyframe animation: center → off-screen
                    // The custom CanTween implementation creates the cubic path
                    transform_seq.set(keyframes![
                        (
                            CardEffects {
                                x_px: 0.0,
                                y_px: 0.0,
                                z_px: 0.0,
                                rot_x_deg: 0.0,
                                rot_y_deg: 0.0,
                                scale_x: 1.0,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: 0.0,
                            },
                            0.0
                        ),
                        (
                            CardEffects {
                                x_px: to_x_px,
                                y_px: 0.0,
                                z_px: adj_to_z,
                                rot_x_deg: 0.0,
                                rot_y_deg: adj_to_rot,
                                scale_x: stretch_end,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM * leave.perspective_scale,
                                blur: CARD_MAX_BLUR,
                            },
                            animation::CARD_LEAVE_BASE_MS,
                            EaseIn
                        )
                    ]);
                    anim_resume();
                    log::debug!("Card: LEAVE_TRAVEL3D edge_ratio={edge_ratio:.2} ms={} rot_adj={adj_to_rot}", animation::CARD_LEAVE_BASE_MS);
                }
                CardAnimation::EdgeEnter3D {
                    side,
                    offset_px,
                    depth_z_px,
                    yaw_deg,
                } => {
                    set_current_edge_side(Some(side));
                    // Use provided offsets directly (no window-based minimum adjustments)
                    let (from_x, from_y) = match side {
                        EdgeSide::Top => (0.0, -offset_px),
                        EdgeSide::Bottom => (0.0, offset_px),
                        EdgeSide::Left => (-offset_px, 0.0),
                        EdgeSide::Right => (offset_px, 0.0),
                    };
                    log::debug!(
                    "Card EdgeEnter3D: side={:?}, offset_px={}, depth_z_px={}, yaw_deg={} -> from_x={}, from_y={}",
                    side, offset_px, depth_z_px, yaw_deg, from_x, from_y
                );
                    anim_ms_sig.set(animation::CARD_ENTER_SHORT_MS);
                    transform_seq.set(keyframes![
                        (
                            CardEffects {
                                x_px: from_x,
                                y_px: from_y,
                                z_px: depth_z_px,
                                rot_x_deg: 0.0,
                                rot_y_deg: yaw_deg,
                                scale_x: 1.5,
                                scale_y: 0.1,
                                perspective: CARD_PERSPECTIVE_CM * 0.9,
                                blur: CARD_MAX_BLUR,
                            },
                            0.0
                        ),
                        (
                            CardEffects {
                                x_px: 0.0,
                                y_px: 0.0,
                                z_px: 0.0,
                                rot_x_deg: 0.0,
                                rot_y_deg: 0.0,
                                scale_x: 1.0,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: 0.0,
                            },
                            animation::CARD_ENTER_SHORT_MS,
                            EaseOut
                        )
                    ]);
                    anim_resume();
                }
                CardAnimation::EdgeLeave3D {
                    side,
                    offset_px,
                    depth_z_px,
                    yaw_deg,
                } => {
                    set_current_edge_side(Some(side));
                    // Use provided offsets directly; no automatic expansion to screen edge
                    let (to_x, to_y) = match side {
                        EdgeSide::Top => (0.0, -offset_px),
                        EdgeSide::Bottom => (0.0, offset_px),
                        EdgeSide::Left => (-offset_px, 0.0),
                        EdgeSide::Right => (offset_px, 0.0),
                    };
                    anim_ms_sig.set(animation::CARD_LEAVE_SHORT_MS);
                    transform_seq.set(keyframes![
                        (
                            CardEffects {
                                x_px: 0.0,
                                y_px: 0.0,
                                z_px: 0.0,
                                rot_x_deg: 0.0,
                                rot_y_deg: 0.0,
                                scale_x: 1.0,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM,
                                blur: 0.0,
                            },
                            0.0
                        ),
                        (
                            CardEffects {
                                x_px: to_x,
                                y_px: to_y,
                                z_px: depth_z_px,
                                rot_x_deg: 0.0,
                                rot_y_deg: yaw_deg,
                                scale_x: 1.0,
                                scale_y: 1.0,
                                perspective: CARD_PERSPECTIVE_CM * 0.9,
                                blur: CARD_MAX_BLUR,
                            },
                            animation::CARD_LEAVE_SHORT_MS,
                            EaseIn
                        )
                    ]);
                    anim_resume();
                }
            }
        }
    });

    let transform_style = move || {
        let s = transform_seq().now();
        format!(
            "translate3d({}px, {}px, {}px) rotateX({}deg) rotateY({}deg) scale({},{})",
            s.x_px, s.y_px, s.z_px, s.rot_x_deg, s.rot_y_deg, s.scale_x, s.scale_y
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

    let transform_origin = move || {
        match current_edge_side() {
            Some(EdgeSide::Left | EdgeSide::Right) => "50% 50%", // Center origin for left/right to prevent Y movement
            _ => "50% 100%", // Bottom center for top/bottom and default
        }
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
            <div
                class=class
                style:transform=transform_style
                style:transform-origin=transform_origin
            >
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
