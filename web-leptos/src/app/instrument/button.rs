use app_core::geometry::Rect;
use leptos::*;


#[component]
pub fn ButtonComponent(
    #[prop(into)] layout_rect: Signal<Rect>,
    #[prop(into, optional)] f_n: Option<usize>,
) -> impl IntoView {
    let style = move || format!(r#"
    width: {}px; 
    height: {}px; 
    top: {}px;
    left: {}px;
    border-radius: {}px;
    "#, 
        layout_rect().width(),
        layout_rect().height(),
        layout_rect().top_left().y,
        layout_rect().top_left().x,
        layout_rect().width()/2.0,
    );

    view! {
        <button class="instrument-button absolute bg-black dark:bg-red text-red dark:text-black"
            style=style
        >
            <Show when=move || f_n.is_some()>
                {f_n}
            </Show>
        </button>
    }
}
