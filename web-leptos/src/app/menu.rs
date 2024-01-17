use leptos::*;

use super::red_card::RedCardComponent;
use app_core::instrument::layout::MenuPosition;
use app_core::{Activity, Event};

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
      <RedCardComponent style=style position=position>
        <Show when={move|| expanded}> 
            <h1 class="text-4xl my-auto text-center">{"Red Siren β"}</h1>
        </Show>
        <button class=btn_class on:click=move|_| menu_ev(Activity::Play)>
            {play_pause}
        </button>
        <Show when={move|| expanded}> 
            {notice} 
        </Show>
        <button class=btn_class on:click=move|_| menu_ev(Activity::Tune)>
            {"Tune"}
        </button>
        <Show when={move|| expanded}>
            <button class=btn_class on:click=move|_| menu_ev(Activity::Listen)>
                {"Listen"}
            </button>
            <button class=btn_class on:click=move|_| menu_ev(Activity::About)>
                {"About"}
            </button>
        </Show>
      </RedCardComponent>
    }
}
