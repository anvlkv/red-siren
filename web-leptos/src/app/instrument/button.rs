use app_core::geometry::Rect;
use leptos::*;


#[component]
pub fn ButtonComponent(
    #[prop(into)] layout_rect: Signal<Rect>,
    #[prop(into, optional)] activation: Signal<f32>,
    children: Children,
) -> impl IntoView {
    let style = move || format!(r#"
    width: {}px; 
    height: {}px; 
    top: {}px;
    left: {}px;
    border-radius: {}px;
    box-shadow: 0 0 0 {}px #36454F, 0 0 0 {}px #E44D2E;
    "#, 
        layout_rect().width(),
        layout_rect().height(),
        layout_rect().top_left().y,
        layout_rect().top_left().x,
        layout_rect().width()/2.0,
        3.0 - 3.0 * activation(),
        3.0 - 3.0 * -activation()
    );

    view! {
        <button class="instrument-button absolute bg-black dark:bg-red text-red dark:text-gray"
            style=style
        >
            {children()}
        </button>
    }
}
