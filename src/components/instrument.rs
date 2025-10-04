mod keyboard;
mod strings;

use leptos::prelude::*;

use keyboard::*;
use strings::*;

#[component]
pub fn Instrument() -> impl IntoView {
    view! {
        <div class="relative w-screen h-screen overflow-hidden">
            <InstrumentStrings attr:class="absolute right-0 bottom-0 stroke-black dark:stroke-red bg-none" />
            <Keyboard attr:class="absolute inset-0" />
        </div>
    }
}
