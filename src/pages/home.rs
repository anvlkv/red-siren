use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

use leptos::prelude::*;
use leptos_use::{use_window_size, UseWindowSizeReturn};
use shared::RouteId;

use crate::components::{AppearAnimationConfig, ContentPage, Menu};

static HOME_APPEAR_PLAYED: OnceLock<AtomicBool> = OnceLock::new();
const APPEAR_ANIMATION_DURATION_MS: f64 = 800.0;

#[component]
pub fn Home() -> impl IntoView {
    let UseWindowSizeReturn { height, .. } = use_window_size();

    let appear_config = move || {
        let h = height.get_untracked();
        AppearAnimationConfig {
            base_height: h,
            base_y_px: h * 1.5,
            tilt_x_from_deg: -60.0,
            base_ms: APPEAR_ANIMATION_DURATION_MS,
            played_flag: &HOME_APPEAR_PLAYED,
        }
    };

    view! {
        <>
            {move || {
                view! {
                    <ContentPage
                        route_id=RouteId::Home
                        appear_animation_config=appear_config()
                        title="Red Siren"
                    >
                        <Menu />
                    </ContentPage>
                }
            }}
        </>
    }
}
