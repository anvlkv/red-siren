use std::time::Duration;

use keyframe::{functions::EaseInCubic, keyframes, AnimationSequence};
use keyframe_derive::CanTween;
use leptos::prelude::*;
use leptos_use::{
    use_prefers_reduced_motion, use_raf_fn_with_options, utils::Pausable, UseRafFnCallbackArgs,
    UseRafFnOptions,
};

const PERSPECTIVE_CM: f64 = 60.0;
const FLIP_ANIMATION_DURATION_MS: f64 = 600.0;
const APPEAR_ANIMATION_DURATION_MS: f64 = 800.0;
const APPEAR_PERSPECTIVE_CM: f64 = 60.0;

#[derive(Debug, Default, Clone, Copy, CanTween)]
struct RotY {
    y_deg: f32,
}

#[derive(Debug, Default, Clone, Copy, CanTween)]
struct Appear {
    x_px: f32,
    y_px: f32,
    tilt_x_deg: f32,
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
    #[prop(optional)] start_animation: Signal<Option<(u64, CardAnimation)>>,
    #[prop(optional)] on_animation_done: Option<Callback<(u64, CardAnimation)>>,
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

    // Animation durations are driven by the incoming CardAnimation variant
    let enter_ms_sig = RwSignal::new(FLIP_ANIMATION_DURATION_MS);
    let leave_ms_sig = RwSignal::new(FLIP_ANIMATION_DURATION_MS);
    let appear_ms_sig = RwSignal::new(APPEAR_ANIMATION_DURATION_MS);

    let enter_seq = RwSignal::new(keyframes![(RotY { y_deg: 0.0 }, 0.0)]);
    let leave_seq = RwSignal::new(keyframes![(RotY { y_deg: 0.0 }, 0.0)]);
    let appear_seq = RwSignal::new(keyframes![(
        Appear {
            x_px: 0.0,
            y_px: 0.0,
            tilt_x_deg: 0.0
        },
        0.0
    )]);

    // Track last seen tx ids to avoid retriggering same animation
    let last_enter_tx = RwSignal::new(None::<(u64, CardAnimation)>);
    let last_leave_tx = RwSignal::new(None::<(u64, CardAnimation)>);
    let last_appear_tx = RwSignal::new(None::<(u64, CardAnimation)>);

    // RAF loops
    let Pausable {
        pause: enter_pause,
        resume: enter_resume,
        is_active: enter_active,
    } = use_raf_fn_with_options(
        move |UseRafFnCallbackArgs { delta, .. }| {
            let reduced = reduced_motion();
            enter_seq.update(|seq| {
                if reduced {
                    seq.advance_to(enter_ms_sig());
                } else {
                    seq.advance_by(delta);
                }
                log::trace!("Card: enter advanced by {delta}ms");
            });
        },
        UseRafFnOptions::default().immediate(false),
    );

    let Pausable {
        pause: leave_pause,
        resume: leave_resume,
        is_active: leave_active,
    } = use_raf_fn_with_options(
        move |UseRafFnCallbackArgs { delta, .. }| {
            let reduced = reduced_motion();
            leave_seq.update(|seq| {
                if reduced {
                    seq.advance_to(leave_ms_sig());
                } else {
                    seq.advance_by(delta);
                }
                log::trace!("Card: leave advanced by {delta}ms");
            });
        },
        UseRafFnOptions::default().immediate(false),
    );

    let Pausable {
        pause: appear_pause,
        resume: appear_resume,
        ..
    } = use_raf_fn_with_options(
        move |UseRafFnCallbackArgs { delta, .. }| {
            let reduced = reduced_motion();
            appear_seq.update(|seq| {
                if reduced {
                    seq.advance_to(appear_ms_sig());
                } else {
                    seq.advance_by(delta);
                }
                log::trace!("Card: appear advanced by {delta}ms");
            });
        },
        UseRafFnOptions::default().immediate(false),
    );

    // Start animation when parent sets a new (tx_id, CardAnimation)
    Effect::new(move |_| {
        if let Some((tx, kind)) = start_animation() {
            match kind {
                CardAnimation::EnterY {
                    from_deg,
                    to_deg,
                    ms,
                } => {
                    if last_enter_tx().map(|(id, _)| id) != Some(tx) {
                        last_enter_tx.set(Some((
                            tx,
                            CardAnimation::EnterY {
                                from_deg,
                                to_deg,
                                ms,
                            },
                        )));
                        enter_ms_sig.set(ms);
                        enter_seq.set(keyframes![
                            (RotY { y_deg: from_deg }, 0.0),
                            (RotY { y_deg: to_deg }, ms, EaseInCubic)
                        ]);
                        // Reset leave contribution
                        leave_seq.set(keyframes![(RotY { y_deg: 0.0 }, 0.0)]);
                        enter_resume();
                        log::debug!("Card: ENTER start tx_id={tx} {from_deg} -> {to_deg}ms={ms}");
                    }
                }
                CardAnimation::LeaveY {
                    from_deg,
                    to_deg,
                    ms,
                } => {
                    if last_leave_tx().map(|(id, _)| id) != Some(tx) {
                        last_leave_tx.set(Some((
                            tx,
                            CardAnimation::LeaveY {
                                from_deg,
                                to_deg,
                                ms,
                            },
                        )));
                        leave_ms_sig.set(ms);
                        leave_seq.set(keyframes![
                            (RotY { y_deg: from_deg }, 0.0),
                            (RotY { y_deg: to_deg }, ms, EaseInCubic)
                        ]);
                        // Reset enter contribution
                        enter_seq.set(keyframes![(RotY { y_deg: 0.0 }, 0.0)]);
                        leave_resume();
                        log::debug!("Card: LEAVE start tx_id={tx} {from_deg} -> {to_deg}ms={ms}");
                    }
                }
                CardAnimation::Appear {
                    x_from_px,
                    y_from_px,
                    tilt_x_from_deg,
                    ms,
                } => {
                    if last_appear_tx().map(|(id, _)| id) != Some(tx) {
                        last_appear_tx.set(Some((
                            tx,
                            CardAnimation::Appear {
                                x_from_px,
                                y_from_px,
                                tilt_x_from_deg,
                                ms,
                            },
                        )));
                        appear_ms_sig.set(ms);
                        appear_seq.set(keyframes![
                            (
                                Appear {
                                    x_px: x_from_px,
                                    y_px: y_from_px,
                                    tilt_x_deg: tilt_x_from_deg
                                },
                                0.0
                            ),
                            (
                                Appear {
                                    x_px: 0.0,
                                    y_px: 0.0,
                                    tilt_x_deg: 0.0
                                },
                                ms,
                                EaseInCubic
                            )
                        ]);
                        appear_resume();
                        log::debug!(
                            "Card: APPEAR start tx_id={tx} trans=({}, {}) tilt={}ms={}",
                            x_from_px,
                            y_from_px,
                            tilt_x_from_deg,
                            ms
                        );
                    }
                }
            }
        }
    });

    let transform_style = move || {
        let a = enter_seq.get().now();
        let b = leave_seq.get().now();
        let c = appear_seq.get().now();
        let total = a.y_deg + b.y_deg;
        let perspective = if c.x_px != 0.0 || c.y_px != 0.0 || c.tilt_x_deg != 0.0 {
            APPEAR_PERSPECTIVE_CM
        } else {
            PERSPECTIVE_CM
        };
        // Clamp X tilt to avoid exaggerated foreshortening which reads like scale.
        let tilt_x = c.tilt_x_deg.clamp(-75.0, 0.0);
        // Ensure a stable baseline: translateZ(0) and scale(1) explicitly set.
        format!(
            "perspective({}cm) translate3d({}px, {}px, 0) translateZ(0) rotate3d(1, 0, 0, {}deg) rotate3d(0, 1, 0, {}deg) scale(1)",
            perspective, c.x_px, c.y_px, tilt_x, total
        )
    };

    // Completion notifications
    Effect::new(move |_| {
        if enter_seq().finished() && enter_active() {
            enter_pause();
            if let Some(((tx, kind), cb)) = last_enter_tx().zip(on_animation_done) {
                set_timeout(move || cb.run((tx, kind)), Duration::from_millis(20));
            }
        }
    });

    Effect::new(move |_| {
        if leave_seq().finished() && leave_active() {
            leave_pause();
            if let Some(((tx, kind), cb)) = last_leave_tx().zip(on_animation_done) {
                set_timeout(move || cb.run((tx, kind)), Duration::from_millis(20));
            }
        }
    });

    Effect::new(move |_| {
        if appear_seq().finished() {
            appear_pause()
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
pub enum CardVariant {
    #[default]
    Elevated,
    Outline,
    Ghost,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardPadding {
    None,
    Sm,
    Md,
    #[default]
    Lg,
}
