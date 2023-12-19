use leptos::*;

use shared::instrument::layout::MenuPosition;
use shared::{Activity, Event};

#[component]
#[allow(unused_variables)]
pub fn MenuComponent(
    #[prop(into)] position: Signal<MenuPosition>,
    #[prop(optional)] expanded: bool,
    #[prop(optional, into)] style: Signal<String>,
    #[prop(optional, into)] playing: Signal<bool>,
) -> impl IntoView {
    let ev_ctx = use_context::<WriteSignal<Event>>().expect("root ev context");
    let menu_ev: Callback<Activity> = (move |activity: Activity| {
        ev_ctx.set(Event::Menu(activity));
    })
    .into();

    let (permission, set_permission) = create_signal(Some(true));

    #[cfg(feature = "browser")]
    {
        use leptos_use::use_window;
        use wasm_bindgen::closure::Closure;

        let window = use_window();

        let cb = Closure::new(move |status: wasm_bindgen::JsValue| {
            let status = web_sys::PermissionStatus::from(status);

            match status.state() {
                web_sys::PermissionState::Granted => set_permission(Some(true)),
                web_sys::PermissionState::Denied => set_permission(Some(false)),
                web_sys::PermissionState::Prompt => set_permission(None),
                _ => set_permission(None),
            }
        });

        let err = Closure::new(move |_| set_permission(None));

        create_effect(move |_| {
            if let Some(navigator) = window.navigator() {
                let permissions = navigator.permissions().expect("permissions api");
                let query = js_sys::Object::new();
                _ = js_sys::Reflect::set(&query, &"name".into(), &"microphone".into())
                    .expect("set property");
                let promise = permissions.query(&query).expect("query permission");
                _ = promise.then(&cb).catch(&err);
            }
        });
    }

    let menu_style = move || {
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

    let menu_class = move || {
        let corner = match position() {
            MenuPosition::TopLeft(_) => "top-left",
            MenuPosition::TopRight(_) => "top-right",
            MenuPosition::BottomLeft(_) => "bottom-left",
            MenuPosition::Center(_) => "center",
        };

        format!("absolute after:bg-black dark:after:bg-red after:rounded-3xl after:shadow-lg bg-black dark:bg-red text-red dark:text-black rounded-3xl shadow-lg menu menu-{corner} ")
    };

    let play_pause = move || if playing() { "Stop" } else { "Play" };
    let btn_class = "w-full rounded-2xl bg-red dark:bg-black text-black dark:text-red text-4xl hover:text-gray dark:hover:text-cinnabar";

    let notice = move || match permission() {
        Some(false) => Some(view! {
            <p class="text-2xl my-auto text-center">{"Red Siren is a noise chime. As an instrument activated by external sounds it requires permission to record audio. Please allow audio recording."}</p>
        }),
        None => Some(view! {
            <p class="text-2xl my-auto text-center">{"Red Siren is a noise chime. Please allow audio recording after you click Play or Tune"}</p>
        }),
        Some(true) => None,
    };

    view! {
      <div class={menu_class} style={menu_style}>
        <h1 class="text-4xl my-auto text-center">{"Red Siren"}</h1>
        <button class=btn_class on:click=move|_| menu_ev(Activity::Play)>
            {play_pause}
        </button>
        {notice}
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
