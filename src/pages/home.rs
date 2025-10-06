use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::OnceLock,
};

use leptos::prelude::*;

use crate::components::{ContentPage, Menu, UiPlacement};

static FIRST_APPEARANCE: OnceLock<AtomicBool> = OnceLock::new();

#[component]
pub fn Home() -> impl IntoView {
    let first_appearance = FIRST_APPEARANCE.get_or_init(|| AtomicBool::new(true));
    let card_animation_direction = if first_appearance.load(Ordering::Relaxed) {
        UiPlacement::Bottom
    } else {
        UiPlacement::Left
    };

    Effect::new(move |_| {
        first_appearance.store(false, Ordering::Relaxed);
    });

    view! {
        <ContentPage title="Red Siren" no_back_button=true card_animation_direction>
            <Menu />
        </ContentPage>
    }
}
