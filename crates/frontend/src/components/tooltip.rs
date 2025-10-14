use crate::components::UiPlacement;
use leptos::prelude::*;

#[component]
pub fn Tooltip(
    #[prop(into)] text: Signal<String>,
    #[prop(optional, into)] placement: Signal<Option<UiPlacement>>,
    children: Children,
) -> impl IntoView {
    // Default to Top when not provided.
    let place = Signal::derive(move || placement().unwrap_or(UiPlacement::Top));

    // Map placement enum to positioning utility classes.
    let tooltip_classes = Signal::derive(move || {
        let places_class = match place() {
            UiPlacement::Bottom => "left-1/2 top-full mt-3 -translate-x-1/2",
            UiPlacement::Left => "right-full top-1/2 -translate-y-1/2 mr-3",
            UiPlacement::Right => "left-full top-1/2 -translate-y-1/2 ml-3",
            UiPlacement::Top => "left-1/2 bottom-full mb-3 -translate-x-1/2",
        };

        format!(
            "pointer-events-none absolute {} rounded-lg bg-black text-red dark:bg-red dark:text-black px-2 py-1 text-base opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity duration-150 whitespace-nowrap shadow-lg z-10",
            places_class,
        )
    });
    let arrow_classes = Signal::derive(move || {
        let places_class = match place() {
            UiPlacement::Bottom => "left-1/2 -translate-x-1/2 -top-2",
            UiPlacement::Left => "top-1/2 -translate-y-1/2 -right-2",
            UiPlacement::Right => "top-1/2 -translate-y-1/2 -left-2",
            UiPlacement::Top => "left-1/2 -translate-x-1/2 -bottom-2",
        };
        format!(
            "absolute w-3 h-3 rotate-45 border-[8px] border-black dark:border-red {}",
            places_class,
        )
    });

    view! {
        <div class="relative group inline-block">
            {children()} <div role="tooltip" class=tooltip_classes>
                {text}
                <span aria-hidden="true" class=arrow_classes></span>
            </div>
        </div>
    }
}
