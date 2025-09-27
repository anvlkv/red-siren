use leptos::prelude::*;
use shared::RouteId;
use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

use crate::components::{AppearAnimationConfig, Menu, Page};

static HOME_APPEAR_PLAYED: OnceLock<AtomicBool> = OnceLock::new();
const APPEAR_ANIMATION_DURATION_MS: f64 = 800.0;

#[component]
pub fn Home() -> impl IntoView {
    let appear_config = AppearAnimationConfig {
        base_height: 900.0,
        base_y_px: 800.0,
        tilt_x_from_deg: -60.0,
        base_ms: APPEAR_ANIMATION_DURATION_MS,
        played_flag: &HOME_APPEAR_PLAYED,
    };

    view! {
        <Page route_id=RouteId::Home appear_animation_config=appear_config title="Red Siren">
            <Menu />
        </Page>
    }
}
