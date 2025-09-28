use std::time::Duration;

use keyframe::{functions::EaseInCubic, keyframes, AnimationSequence};
use keyframe_derive::CanTween;
use leptos::prelude::*;
use leptos_use::{
    use_prefers_reduced_motion, use_raf_fn_with_options, utils::Pausable, UseRafFnCallbackArgs,
    UseRafFnOptions,
};

const PERSPECTIVE_CM: f64 = 60.0;
const APPEAR_PERSPECTIVE_CM: f64 = 60.0;

#[derive(Debug, Default, Clone, Copy, CanTween)]
struct Transform {
    x_px: f32,
    y_px: f32,
    tilt_x_deg: f32,
    rot_y_deg: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum CardAnimation {
    Appear {
        x_from_px: f32,
        y_from_px: f32,
        tilt_x_from_deg: f32,
        ms: f64,
    },
    EnterY {
        from_deg: f32,
        to_deg: f32,
        ms: f64,
    },
    LeaveY {
        from_deg: f32,
        to_deg: f32,
        ms: f64,
    },
}

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
    #[prop(optional)] variant: CardVariant,
    #[prop(optional)] padding: CardPadding,
    #[prop(optional)] rounded: bool,
    #[prop(optional)] full_width: bool,
    #[prop(optional, into)] class: String,

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
                focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2";

    let bg_and_border = match variant {
        CardVariant::Elevated => {
            "\
            bg-red dark:bg-black \
            shadow-xl shadow-gray dark:shadow-cinnabar"
        }
        CardVariant::Outline => {
            "\
            bg-transparent border-2 border-black dark:border-red \
            hover:shadow-sm hover:shadow-gray dark:hover:shadow-cinnabar"
        }
        CardVariant::Ghost => {
            "\
            bg-transparent"
        }
    };

    let rounding = if rounded { "rounded-xl" } else { "rounded-lg" };

    let padding_cls = match padding {
        CardPadding::None => "p-0",
        CardPadding::Sm => "p-3",
        CardPadding::Md => "p-6",
        CardPadding::Lg => "p-8",
    };

    let width_cls = if full_width { "w-full" } else { "w-max" };

    let interactive_cls = if interactive {
        "\
        cursor-pointer \
        hover:scale-[1.01] \
        active:scale-[0.995]"
    } else {
        "cursor-default"
    };

    let class = format!(
        "{base} {bg_and_border} {rounding} {padding_cls} {width_cls} {interactive_cls} {class}"
    );

    // Built-in rotateY animation plumbing (imperative)
    let reduced_motion = use_prefers_reduced_motion();

    let anim_ms_sig = RwSignal::new(0.0_f64);
    let transform_seq = RwSignal::new(keyframes![(
        Transform {
            x_px: 0.0,
            y_px: 0.0,
            tilt_x_deg: 0.0,
            rot_y_deg: 0.0
        },
        0.0
    )]);

    // RAF loops
    let Pausable {
        pause: anim_pause,
        resume: anim_resume,
        is_active: anim_active,
    } = use_raf_fn_with_options(
        move |UseRafFnCallbackArgs { delta, .. }| {
            let reduced = reduced_motion();
            transform_seq.update(|seq| {
                if reduced {
                    seq.advance_to(anim_ms_sig());
                } else {
                    let delta = delta.min(32.0);
                    seq.advance_by(delta);
                }
            });
        },
        UseRafFnOptions::default().immediate(false),
    );

    // Start animation when parent sets a new (tx_id, CardAnimation)
    Effect::new(move |_| {
        if let Some(animation) = start_animation() {
            match animation {
                CardAnimation::EnterY {
                    from_deg,
                    to_deg,
                    ms,
                } => {
                    anim_ms_sig.set(ms);
                    transform_seq.set(keyframes![
                        (
                            Transform {
                                x_px: 0.0,
                                y_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: from_deg
                            },
                            0.0
                        ),
                        (
                            Transform {
                                x_px: 0.0,
                                y_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: to_deg
                            },
                            ms,
                            EaseInCubic
                        )
                    ]);
                    anim_resume();
                    log::debug!("Card: ENTER start {from_deg} -> {to_deg}ms={ms}");
                }
                CardAnimation::LeaveY {
                    from_deg,
                    to_deg,
                    ms,
                } => {
                    anim_ms_sig.set(ms);
                    transform_seq.set(keyframes![
                        (
                            Transform {
                                x_px: 0.0,
                                y_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: from_deg
                            },
                            0.0
                        ),
                        (
                            Transform {
                                x_px: 0.0,
                                y_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: to_deg
                            },
                            ms,
                            EaseInCubic
                        )
                    ]);
                    anim_resume();
                    log::debug!("Card: LEAVE start {from_deg} -> {to_deg}ms={ms}");
                }
                CardAnimation::Appear {
                    x_from_px,
                    y_from_px,
                    tilt_x_from_deg,
                    ms,
                } => {
                    anim_ms_sig.set(ms);
                    transform_seq.set(keyframes![
                        (
                            Transform {
                                x_px: x_from_px,
                                y_px: y_from_px,
                                tilt_x_deg: tilt_x_from_deg,
                                rot_y_deg: 0.0
                            },
                            0.0
                        ),
                        (
                            Transform {
                                x_px: 0.0,
                                y_px: 0.0,
                                tilt_x_deg: 0.0,
                                rot_y_deg: 0.0
                            },
                            ms,
                            EaseInCubic
                        )
                    ]);
                    anim_resume();
                    log::debug!(
                        "Card: APPEAR start trans=({}, {}) tilt={}ms={}",
                        x_from_px,
                        y_from_px,
                        tilt_x_from_deg,
                        ms
                    );
                }
            }
        }
    });

    let transform_style = move || {
        let s = transform_seq.get().now();
        let perspective = if s.x_px != 0.0 || s.y_px != 0.0 || s.tilt_x_deg != 0.0 {
            APPEAR_PERSPECTIVE_CM
        } else {
            PERSPECTIVE_CM
        };
        // Clamp X tilt to avoid exaggerated foreshortening which reads like scale.
        let tilt_x = s.tilt_x_deg.clamp(-75.0, 0.0);
        // Ensure a stable baseline: translateZ(0) and scale(1) explicitly set.
        format!(
            "perspective({}cm) rotate3d(1, 0, 0, {}deg) rotate3d(0, 1, 0, {}deg) translate3d({}px, {}px, 0) translateZ(0) scale(1)",
            perspective, tilt_x, s.rot_y_deg, s.x_px, s.y_px
        )
    };

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
            class=class
            role=if interactive { "button" } else { "group" }
            tabindex=if interactive { Some("0") } else { None }
            class:will-change-transform=true
            style:transform=transform_style
            style:transform-origin="50% 100%"
            style:backface-visibility="hidden"
            style:contain="layout paint"
        >
            <div class="card-body contents">{children()}</div>
        </div>
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CardVariant {
    #[default]
    Elevated,
    Outline,
    Ghost,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CardPadding {
    None,
    Sm,
    Md,
    #[default]
    Lg,
}
