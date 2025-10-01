mod keyboard;
mod strings;

use leptos::prelude::*;

use keyboard::*;
use strings::*;

use crate::util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn};

#[component]
pub fn Instrument() -> impl IntoView {
    let UseTauriResourceReturn { data, .. } =
        use_tauri_resource::<shared::instrument::Layout>(shared::instrument::events::LAYOUT);

    let layout = Signal::derive(move || data().unwrap_or_default());

    view! {
        <div class="relative w-screen h-screen">
            <InstrumentStrings
                layout
                attr:class="absolute w-full h-full stroke-black dark:stroke-red bg-none"
                attr:width=move || format!("{}px", layout().space.x)
                attr:height=move || format!("{}px", layout().space.y)
            />
            <Keyboard layout attr:class="w-full h-full" />
        </div>
    }
}
