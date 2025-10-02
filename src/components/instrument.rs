mod keyboard;
mod strings;

use leptos::prelude::*;

use keyboard::*;
use strings::*;

use crate::util::layout_context::{expect_layout_contex, LayoutContextReturn};

#[component]
pub fn Instrument() -> impl IntoView {
    let LayoutContextReturn { space, .. } = expect_layout_contex();
    view! {
        <div class="relative w-screen h-screen">
            <InstrumentStrings
                attr:class="absolute w-full h-full stroke-black dark:stroke-red bg-none"
                attr:width=move || format!("{}px", space().x)
                attr:height=move || format!("{}px", space().y)
            />
            <Keyboard attr:class="w-full h-full" />
        </div>
    }
}
