use leptos::*;

use app_core::instrument::layout::MenuPosition;

#[component]
pub fn RedCardComponent(
    #[prop(into)] position: Signal<MenuPosition>,
    #[prop(optional, into)] style: Signal<String>,
    children: Children
) -> impl IntoView {

    let card_style = move || {
        let pos = position();
        let rect = pos.rect();
        let style = style();
        format!(
            r#"
            width: {}px; 
            height: {}px; 
            top: {}px; 
            left: {}px;
            {style}
            "#,
            rect.width(),
            rect.height(),
            rect.top_left().y,
            rect.top_left().x,
        )
    };

    let card_class = move || {
        let corner = match position() {
            MenuPosition::TopLeft(_) => "top-left",
            MenuPosition::TopRight(_) => "top-right",
            MenuPosition::BottomLeft(_) => "bottom-left",
            MenuPosition::Center(_) => "center",
        };

        format!("absolute after:bg-black dark:after:bg-red after:rounded-3xl after:shadow-lg bg-black dark:bg-red text-red dark:text-black rounded-3xl shadow-lg menu menu-{corner} ")
    };

    view! {
      <div class={card_class} style={card_style}>
        {children()}
      </div>
    }
}
