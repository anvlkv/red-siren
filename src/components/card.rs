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
const APPEAR_PERSPECTIVE_CM: f64 = 10.0;

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
///     start_enter_tx=Some(enter_tx_signal)
///     start_leave_tx=Some(leave_tx_signal)
///     on_enter_done=Some(Callback::new(|tx| notify_enter_done(tx)))
///     on_leave_done=Some(Callback::new(|tx| notify_leave_done(tx)))
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

    // Imperative animation controls (optional):
    // Set a new tx_id to start an animation. Card will run animations and then call on_*_done if provided.
    #[prop(optional)] start_appear_tx: Signal<Option<u64>>,
    #[prop(optional)] start_enter_tx: Signal<Option<u64>>,
    #[prop(optional)] start_leave_tx: Signal<Option<u64>>,
    #[prop(optional)] on_enter_done: Option<Callback<u64>>,
    #[prop(optional)] on_leave_done: Option<Callback<u64>>,
    // Animation tuning (rotateY)
    #[prop(optional)] enter_from_deg: f32,
    #[prop(optional)] enter_to_deg: f32,
    #[prop(optional)] enter_ms: f64,
    #[prop(optional)] leave_from_deg: f32,
    #[prop(optional)] leave_to_deg: f32,
    #[prop(optional)] leave_ms: f64,
    // Animation tuning (appear)
    #[prop(optional)] appear_x_from_px: f32,
    #[prop(optional)] appear_y_from_px: f32,
    #[prop(optional)] appear_tilt_x_from_deg: f32,
    #[prop(optional)] appear_ms: f64,
) -> impl IntoView {
    // Base layout and typography colors tuned to the existing theme
    let base = "text-black dark:text-red \
                transition-all duration-200 \
                focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 \
                will-change-transform";

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

    let ef = if enter_from_deg == 0.0 {
        -90.0
    } else {
        enter_from_deg
    };
    let et = enter_to_deg;
    let ems = if enter_ms == 0.0 {
        FLIP_ANIMATION_DURATION_MS
    } else {
        enter_ms
    };

    let lf = leave_from_deg;
    let lt = if leave_to_deg == 0.0 {
        90.0
    } else {
        leave_to_deg
    };
    let lms = if leave_ms == 0.0 {
        FLIP_ANIMATION_DURATION_MS
    } else {
        leave_ms
    };

    // Appear animation defaults (replicates old Home animation)
    let ax = appear_x_from_px;
    let ay = if appear_y_from_px == 0.0 {
        1000.0
    } else {
        appear_y_from_px
    };
    let atilt = if appear_tilt_x_from_deg == 0.0 {
        -120.0
    } else {
        appear_tilt_x_from_deg
    };
    let ams = if appear_ms == 0.0 {
        APPEAR_ANIMATION_DURATION_MS
    } else {
        appear_ms
    };

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
    let last_enter_tx = RwSignal::new(None::<u64>);
    let last_leave_tx = RwSignal::new(None::<u64>);
    let last_appear_tx = RwSignal::new(None::<u64>);

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
                    seq.advance_to(ems);
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
                    seq.advance_to(lms);
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
                    seq.advance_to(ams);
                } else {
                    seq.advance_by(delta);
                }
                log::trace!("Card: appear advanced by {delta}ms");
            });
        },
        UseRafFnOptions::default().immediate(false),
    );

    // Start ENTER when parent sets a new tx_id
    Effect::new(move |_| {
        if let Some(tx) = start_enter_tx() {
            if last_enter_tx() != Some(tx) {
                last_enter_tx.set(Some(tx));
                enter_seq.set(keyframes![
                    (RotY { y_deg: ef }, 0.0),
                    (RotY { y_deg: et }, ems, EaseInCubic)
                ]);
                // Reset leave contribution
                leave_seq.set(keyframes![(RotY { y_deg: 0.0 }, 0.0)]);
                enter_resume();
                log::debug!("Card: ENTER start tx_id={tx} {ef} -> {et}ms={ems}");
            }
        }
    });

    // Start APPEAR when parent sets a new tx_id
    Effect::new(move |_| {
        if let Some(tx) = start_appear_tx() {
            if last_appear_tx() != Some(tx) {
                last_appear_tx.set(Some(tx));
                appear_seq.set(keyframes![
                    (
                        Appear {
                            x_px: ax,
                            y_px: ay,
                            tilt_x_deg: atilt
                        },
                        0.0
                    ),
                    (
                        Appear {
                            x_px: 0.0,
                            y_px: 0.0,
                            tilt_x_deg: 0.0
                        },
                        ams,
                        EaseInCubic
                    )
                ]);
                appear_resume();
                log::debug!(
                    "Card: APPEAR start tx_id={tx} trans=({}, {}) tilt={}ms={ams}",
                    ax,
                    ay,
                    atilt
                );
            }
        }
    });

    // Start LEAVE when parent sets a new tx_id
    Effect::new(move |_| {
        if let Some(tx) = start_leave_tx() {
            if last_leave_tx() != Some(tx) {
                last_leave_tx.set(Some(tx));
                leave_seq.set(keyframes![
                    (RotY { y_deg: lf }, 0.0),
                    (RotY { y_deg: lt }, lms, EaseInCubic)
                ]);
                // Reset enter contribution
                enter_seq.set(keyframes![(RotY { y_deg: 0.0 }, 0.0)]);
                leave_resume();
                log::debug!("Card: LEAVE start tx_id={tx} {lf} -> {lt}ms={lms}");
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
        format!(
            "perspective({}cm) translate3d({}px, {}px, 0) rotate3d(1, 0, 0, {}deg) rotate3d(0, 1, 0, {}deg)",
            perspective, c.x_px, c.y_px, c.tilt_x_deg, total
        )
    };

    // Completion notifications
    Effect::new(move |_| {
        if enter_seq().finished() && enter_active() {
            enter_pause();
            if let Some((tx, cb)) = last_enter_tx().zip(on_enter_done) {
                set_timeout(move || cb.run(tx), Duration::from_millis(20));
            }
        }
    });

    Effect::new(move |_| {
        if leave_seq().finished() && leave_active() {
            leave_pause();
            if let Some((tx, cb)) = last_leave_tx().zip(on_leave_done) {
                set_timeout(move || cb.run(tx), Duration::from_millis(20));
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
            style:transform=transform_style
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
