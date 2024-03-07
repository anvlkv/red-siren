use app_core::UnitState;
use futures::channel::mpsc::Sender;
use leptos::*;
use leptos_router::{use_location, Location};
use leptos_use::{use_event_listener, use_media_query, use_window};

use super::area::Area;
use super::intro::Intro;
use super::objects::Objects;
use crate::app::animation_clock::AnimationClock;
use crate::app::core_bindings;
use crate::util::use_dpi;

#[derive(Clone)]
pub struct CoreContext(
    pub ReadSignal<app_core::ViewModel>,
    pub WriteSignal<Option<app_core::Event>>,
);

#[component]
pub fn RedSirenCore() -> impl IntoView {
    let core = core_bindings::new();
    let clock = AnimationClock::new();
    let Location { pathname, .. } = use_location();
    let view_rw_signal = create_rw_signal(core.view());
    let render = view_rw_signal.write_only();
    let view = view_rw_signal.read_only();

    let (event, set_event) = create_signal(None);

    create_effect(move |prev| {
        let activity = match pathname.get().as_str() {
            "/tune" => app_core::Activity::Tune,
            "/play" => app_core::Activity::Play,
            "/listen" => app_core::Activity::Listen,
            "/about" => app_core::Activity::About,
            "/" => app_core::Activity::Intro,
            _ => app_core::Activity::Unknown,
        };
        if let Some(_) = prev {
            set_event(Some(app_core::Event::Navigation(activity)))
        } else {
            set_event(Some(app_core::Event::InitialNavigation(activity)));
        }
        activity
    });

    let navigate = leptos_router::use_navigate();
    let navigate_cb = Callback::new(move |path| navigate(path, Default::default()));

    let animate = Callback::new(move |sender: Option<Sender<f64>>| match sender {
        Some(sender) => clock.add_sender(sender),
        None => clock.clear(),
    });

    create_effect(move |_| {
        if let Some(ev) = event.get() {
            core_bindings::update(&core, ev, render, navigate_cb, animate);
        }
    });

    let reduced_motion = use_media_query("(prefers-reduced-motion)");
    create_effect(move |_| {
        let reduced_motion = reduced_motion.get();
        set_event.set(Some(app_core::Event::Visual(
            app_core::VisualEV::SetReducedMotion(reduced_motion),
        )));
    });

    let dark_mode = use_media_query("(prefers-color-scheme: dark)");
    create_effect(move |_| {
        let dark_mode = dark_mode.get();
        set_event.set(Some(app_core::Event::Visual(
            app_core::VisualEV::SetDarkMode(dark_mode),
        )));
    });

    provide_context(CoreContext(view, set_event));
    let (size, set_size) = create_signal((0, 0));
    let window = use_window();
    _ = use_event_listener(window.clone(), leptos::ev::resize, move |_| {
        let body = window.document().body().unwrap();
        let new_size = (body.client_width(), body.client_height());
        set_size.set(new_size);
    });

    let window = use_window();
    create_effect(move |_| {
        let body = window.document().body().unwrap();
        set_size.set((body.client_width(), body.client_height()));
    });

    create_effect(move |_| {
        let (width, height) = size.get();
        set_event(Some(app_core::Event::Visual(app_core::VisualEV::Resize(
            width as f64,
            height as f64,
        ))));
    });

    create_effect(move |_| {
        set_event(Some(app_core::Event::Visual(
            app_core::VisualEV::SafeAreaResize(50.0, 50.0, 50.0, 50.0),
        )));
    });

    let dpi = use_dpi(vec![120, 160, 240, 320, 480, 640]);
    create_effect(move |_| {
        set_event(Some(app_core::Event::Visual(
            app_core::VisualEV::SetDensity(dpi() as f64),
        )));
    });

    // draft
    let on_click = move |_| match view().unit_state {
        UnitState::None => set_event(Some(app_core::Event::StartAudioUnit)),
        UnitState::Playing => set_event(Some(app_core::Event::Pause)),
        UnitState::Paused => set_event(Some(app_core::Event::Resume)),
    };

    view! {
        <div class="red-siren-core-view" on:click=on_click>
            <Intro opacity=Signal::derive(move|| view().visual.intro_opacity)/>
            <Area>
                <text y="50" x="50" fill="black">{move || format!("{:?}", view().unit_state)}</text>
                <Objects/>
            </Area>
        </div>
    }
}
