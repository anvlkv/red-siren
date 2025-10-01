use leptos::prelude::*;

#[component]
pub fn Tooltip(
    #[prop(into)] text: Signal<String>,
    #[prop(optional, into)] placement: Option<String>,
    children: Children,
) -> impl IntoView {
    let placement = placement.unwrap_or_else(|| "top".to_string());

    // Compute positioning for tooltip and its callout arrow based on placement.
    let (pos_classes, arrow_pos) = match placement.as_str() {
        "bottom" => (
            "left-1/2 top-full mt-3 -translate-x-1/2",
            "left-1/2 -translate-x-1/2 -top-2",
        ),
        "left" => (
            "right-full top-1/2 -translate-y-1/2 mr-3",
            "top-1/2 -translate-y-1/2 -right-2",
        ),
        "right" => (
            "left-full top-1/2 -translate-y-1/2 ml-3",
            "top-1/2 -translate-y-1/2 -left-2",
        ),
        _ => (
            "left-1/2 bottom-full mb-3 -translate-x-1/2",
            "left-1/2 -translate-x-1/2 -bottom-2",
        ),
    };

    view! {
        <div class="relative group inline-block">
            {children()}
            <div
                role="tooltip"
                class=format!(
                    "pointer-events-none absolute {} rounded-lg bg-black text-red dark:bg-red dark:text-black px-2 py-1 text-base opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity duration-150 whitespace-nowrap shadow-lg z-10",
                    pos_classes,
                )
            >
                {text}
                <span
                    aria-hidden="true"
                    class=format!(
                        "absolute w-3 h-3 rotate-45 border-[8px] border-black dark:border-red {}",
                        arrow_pos,
                    )
                ></span>
            </div>
        </div>
    }
}
