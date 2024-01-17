use leptos::*;
use leptos_meta::Title;

use app_core::instrument::layout::MenuPosition;
use app_core::{intro, Activity, Event};

use super::intro::SplashPicture;

#[component]
pub fn AboutComponent(vm: Signal<intro::IntroVM>) -> impl IntoView {
    let position = Signal::derive(move || vm().layout.menu_position);

    view! {
        <div class="h-full w-full bg-red dark:bg-black splash">
            <SplashPicture/>
            <AboutContent position=position/>
        </div>
    }
}

#[component]
#[allow(unused_variables)]
pub fn AboutContent(#[prop(into)] position: Signal<MenuPosition>) -> impl IntoView {
    let ev_ctx = use_context::<WriteSignal<Event>>().expect("root ev context");
    let exit_ev: Callback<()> = (move |_| {
        ev_ctx.set(Event::Menu(Activity::Intro));
    })
    .into();

    let about_style = move || {
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

    let about_class =
        "absolute bg-black dark:bg-red text-red dark:text-black rounded-3xl shadow-lg about "
            .to_string();

    let btn_class = "w-full rounded-2xl bg-red dark:bg-black text-black dark:text-red text-4xl hover:text-gray dark:hover:text-cinnabar";

    view! {
      <div class={about_class} style={about_style}>
        <Title text="Red Siren - About"/>
        <h2 class="text-4xl my-auto text-center italic">{"About the Red Siren β"}</h2>
        <p class="text-center">{"Red Siren is a noise chime."}</p>
        <dl>
            <dt>{"Red"}</dt>
                <dd>{"The color red and its many meanings."}</dd>
            <dt>{"Siren"}</dt>
                <dd>{"Siren - the mythical creature, but also the alarm."}</dd>
            <dt>{"is"}</dt>
                <dd>{"It exists right now."}</dd>
            <dt>{"a"}</dt>
                <dd>{"It's a choice, one of many, and therefore any."}</dd>
            <dt>{"noise"}</dt>
                <dd>{"Random or unwanted sounds."}</dd>
            <dt>{"chime"}</dt>
                <dd>{"The musical instrument."}</dd>
        </dl>

        <button class=btn_class on:click=move|_| exit_ev(())>
            {"Clear"}
        </button>
      </div>
    }
}
