mod context;
mod debug;
mod element;
mod keyboard;
mod spectrum;
mod strings;

use crate::{
    components::intro::consts::INTRO_FADE_DURATION_MS,
    util::layout_context::{expect_layout_context, LayoutContextReturn},
};
use leptos::prelude::*;
use leptos_styling::style_sheet;

style_sheet!(
    instrument_animations,
    "src/components/instrument/animations.css",
    "instrument_animations"
);

pub use context::{expect_instrument_context, provide_instrument_context};

use keyboard::*;
use spectrum::*;
use strings::*;

#[component]
pub fn Instrument(#[prop(into, optional)] editor: Signal<bool>) -> impl IntoView {
    provide_instrument_context();
    let LayoutContextReturn { space, .. } = expect_layout_context();
    view! {
        <div
            class=format!(
                "{} {} relative overflow-hidden isolate bg-red dark:bg-black",
                instrument_animations::INSTRUMENT_SCENE_VT_BOTTOM,
                instrument_animations::INSTRUMENT_SCENE_ENTER,
            )
            style=move || {
                format!(
                    "--inst-crossfade-duration: {}ms; width: {}px; height: {}px;",
                    INTRO_FADE_DURATION_MS / 2.0,
                    space().x,
                    space().y,
                )
            }
        >
            <SpectrumViz attr:class="absolute h-full w-full mix-blend-plus-lighter blur-xs opacity-75" />
            <InstrumentStrings attr:class="absolute h-full w-auto right-0 bottom-0" />
            <Keyboard attr:class="absolute inset-0" />
            <Show when=move || editor()>
                <debug::DebugOverlay />
            </Show>
        </div>
    }
}
