use leptos::prelude::*;

#[component]
pub fn Fold(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! {
        <details class="fold">
            <summary class="text-xl leading-tight text-black dark:text-red mb-1 opacity-80">
                {title}
            </summary>
            {children()}
        </details>
    }
}
