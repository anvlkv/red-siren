use leptos::*;

use shared::instrument::layout::MenuPosition;
use shared::{Activity, Event};

#[component]
pub fn MenuComponent(
    #[prop(into)] position: Signal<MenuPosition>,
    #[prop(optional)] expanded: bool,
    #[prop(into)] playing: Signal<bool>,
) -> impl IntoView {
    let ev_ctx = use_context::<WriteSignal<Event>>().expect("root ev context");
    let menu_ev: Callback<Activity> = (move |activity: Activity| {
        ev_ctx.set(Event::Menu(activity));
    })
    .into();

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

        format!("absolute bg-black dark:bg-red text-red dark:text-black rounded-3xl shadow-lg menu menu-{corner} ")
    };

    let play_pause = move || if playing() { "Stop" } else { "Play" };
    let btn_class = "w-full rounded-2xl bg-red dark:bg-black text-black dark:text-red text-4xl hover:text-gray dark:hover:text-cinnabar";

    view! {
      <div class={menu_class} style={menu_style}>
        <h1 class="text-4xl text-center font-bold">{"Red Siren"}</h1>
        <button class=btn_class on:click=move|_| menu_ev(Activity::Play)>
            {play_pause}
        </button>
        <p class="text-2xl text-center">{"Red Siren is a noise chime. Please allow audio recording after you click Play or Tune"}</p>
        <button class=btn_class on:click=move|_| menu_ev(Activity::Tune)>
            {"Tune"}
        </button>
        <button class=btn_class on:click=move|_| menu_ev(Activity::Listen)>
            {"Listen"}
        </button>
        <button class=btn_class on:click=move|_| menu_ev(Activity::About)>
            {"About"}
        </button>
      </div>
    }
}
