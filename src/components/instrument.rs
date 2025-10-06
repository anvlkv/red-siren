mod context;
mod keyboard;
mod strings;

use crate::components::intro::consts::INTRO_FADE_DURATION_MS;
use leptos::prelude::*;
use leptos_styling::style_sheet;

style_sheet!(
    instrument_animations,
    "src/components/instrument/animations.css",
    "instrument_animations"
);

pub use context::{expect_instrument_context, provide_instrument_context};

use keyboard::*;
use strings::*;

#[component]
pub fn Instrument() -> impl IntoView {
    provide_instrument_context();
    view! {
        <div
            class=format!(
                "{} relative w-screen h-screen overflow-hidden",
                instrument_animations::INSTRUMENT_SCENE_VT_BOTTOM,
            )
            style=format!("--inst-appear-duration: {}ms;", INTRO_FADE_DURATION_MS)
        >
            <InstrumentStrings attr:class="absolute right-0 bottom-0 stroke-black dark:stroke-red bg-none" />
            <Keyboard attr:class="absolute inset-0" />
        </div>
    }
}
