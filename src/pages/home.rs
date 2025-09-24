use keyframe::{functions::EaseInCubic, keyframes, AnimationSequence};
use keyframe_derive::CanTween;
use leptos::prelude::*;
use leptos_use::{use_prefers_reduced_motion, use_raf_fn, utils::Pausable, UseRafFnCallbackArgs};
use mint::Point2;

use crate::components::Menu;

const ANIMATION_DURATION_MS: f64 = 800.0;

#[derive(Debug, Default, Clone, Copy, CanTween)]
struct HomeMenuAnimationState {
    y_offset: f32,
    x_offset: f32,
    x_tilt: f32,
}

#[component]
pub fn Home(#[prop(into, optional)] initial_position: Option<Point2<f32>>) -> impl IntoView {
    let reduced_motion = use_prefers_reduced_motion();
    let first_frame = match initial_position {
        Some(pos) => HomeMenuAnimationState {
            y_offset: pos.y,
            x_offset: pos.x,
            x_tilt: 0.0,
        },
        None => HomeMenuAnimationState {
            y_offset: 1000.0,
            x_offset: 0.0,
            x_tilt: -120.0,
        },
    };
    let animation_state = RwSignal::new(keyframes![
        (first_frame, 0.0),
        (
            HomeMenuAnimationState::default(),
            ANIMATION_DURATION_MS,
            EaseInCubic
        )
    ]);

    let Pausable { pause, .. } = use_raf_fn(move |UseRafFnCallbackArgs { delta, .. }| {
        let reduced_motion = reduced_motion();

        animation_state.update(|state| {
            if reduced_motion {
                state.advance_to(ANIMATION_DURATION_MS);
            } else {
                state.advance_by(delta);
            }
            log::trace!("advanced animation state by: {delta}");
        });
    });

    Effect::new(move |_| {
        if animation_state().finished() {
            pause()
        }
    });

    let menu_transform = move || {
        let state = animation_state().now();
        format!(
            "perspective(10cm) translate3d({}px, {}px, 0) rotate3d(1, 0, 0, {}deg)",
            state.x_offset, state.y_offset, state.x_tilt
        )
    };

    view! {
        <div class="w-full h-full flex items-center justify-center" style:transform=menu_transform>
            <Menu />
        </div>
    }
}
