mod context;
mod debug;
mod editor;
mod element;
mod keyboard;
mod strings;

use crate::{
    components::intro::consts::INTRO_FADE_DURATION_MS,
    util::layout_context::{expect_layout_contex, LayoutContextReturn},
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
use strings::*;

#[component]
pub fn Instrument(#[prop(into, optional)] editor: Signal<bool>) -> impl IntoView {
    provide_instrument_context();
    let LayoutContextReturn { space, .. } = expect_layout_contex();
    view! {
        <div
            class=format!(
                "{} {} relative overflow-hidden",
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
            <InstrumentStrings attr:class="absolute h-full w-auto right-0 bottom-0 fill-none stroke-gray/40 dark:stroke-cinnabar/40 stroke-[0.5px]" />
            <Keyboard attr:class="absolute inset-0 backdrop-blur-3xl" />
            <Show when=move || editor()>
                <debug::DebugOverlay />
                <editor::EditorOverlay />
            </Show>
        </div>
    }
}
