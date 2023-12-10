use leptos::*;
use shared::instrument::layout::MenuPosition;

#[component]
pub fn MenuComponent(
    #[prop(into)] position: Signal<MenuPosition>,
    #[prop(into)] playing: Signal<bool>,
    #[prop(into)] toggle_playing: Callback<()>,
) -> impl IntoView {
    let menu_style = move || {
        let pos = position();
        let rect = pos.rect();
        format!(
            "width: {}px; height: {}px; top: {}px; left: {}px",
            rect.width(),
            rect.height(),
            rect.top_left().y,
            rect.top_left().x
        )
    };

    let menu_class = move || {
        let corner = match position() {
            MenuPosition::TopLeft(_) => "top-left",
            MenuPosition::TopRight(_) => "top-right",
            MenuPosition::BottomLeft(_) => "bottom-left",
            MenuPosition::Center(_) => "center",
        };

        format!("absolute bg-red menu menu-{corner}")
    };

    let play_pause = move || if playing() { "Stop" } else { "Play" };

    view! {
      <div class={menu_class} style={menu_style}>
        <button on:click=move|_| toggle_playing(())>
            {play_pause}
        </button>
      </div>
    }
}
