use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use leptos::prelude::*;
use leptos_use::{use_window_size, UseWindowSizeReturn};
use shared::RouteId;

use crate::{
    components::{AppearAnimationConfig, ContentPage, Menu},
    util::selective_signal::{use_selective_signal, UseSelectiveSignalReturn},
};

static HOME_APPEAR_PLAYED: OnceLock<AtomicBool> = OnceLock::new();
const APPEAR_ANIMATION_DURATION_MS: f64 = 800.0;

#[component]
pub fn Home() -> impl IntoView {
    let UseWindowSizeReturn { height, .. } = use_window_size();

    _ = HOME_APPEAR_PLAYED.get_or_init(|| AtomicBool::new(false));

    let UseSelectiveSignalReturn {
        value: appear_animation,
        set_source,
        ..
    } = use_selective_signal(
        {
            let h = height.get_untracked();
            AppearAnimationConfig {
                base_height: h,
                base_y_px: h * 1.5,
                tilt_x_from_deg: -60.0,
                base_ms: APPEAR_ANIMATION_DURATION_MS,
                played_flag: &HOME_APPEAR_PLAYED,
            }
        },
        |cfg| {
            let played = cfg.played_flag.get().unwrap();
            !played.load(Ordering::Relaxed)
        },
    );

    Effect::new(move |_| {
        let h = height();
        set_source(AppearAnimationConfig {
            base_height: h,
            base_y_px: h * 1.5,
            tilt_x_from_deg: -60.0,
            base_ms: APPEAR_ANIMATION_DURATION_MS,
            played_flag: &HOME_APPEAR_PLAYED,
        })
    });

    view! {
        <Show when=move || {
            appear_animation().base_height > 0.0
        }>
            {move || {
                view! {
                    <ContentPage
                        route_id=RouteId::Home
                        appear_animation_config=appear_animation()
                        title="Red Siren"
                        no_back_button=true
                    >
                        <Menu />
                    </ContentPage>
                }
            }}
        </Show>
    }
}
