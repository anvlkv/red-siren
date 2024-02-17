use futures::channel::mpsc::Sender;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use leptos_use::{
    use_event_listener, use_media_query, use_timestamp_with_controls_and_options, use_window,
    UseTimestampOptions, UseTimestampReturn,
};
use std::rc::Rc;

mod area;
mod core_bindings;
mod intro;
mod object;
mod audio;

use crate::error_template::{AppError, ErrorTemplate};
use area::Area;
use intro::Intro;
use object::Object;
use audio::Audio;

#[component]
pub fn RootComponent() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/red-siren.css"/>
        <Title text="Red Siren"/>
        <Link rel="icon" type_="image/x-icon" href="/favicon/favicon.ico" />
        <Link rel="apple-touch-icon-precomposed" sizes="57x57" href="/favicon/apple-touch-icon-57x57.png" />
        <Link rel="apple-touch-icon-precomposed" sizes="114x114" href="/favicon/apple-touch-icon-114x114.png" />
        <Link rel="apple-touch-icon-precomposed" sizes="72x72" href="/favicon/apple-touch-icon-72x72.png" />
        <Link rel="apple-touch-icon-precomposed" sizes="144x144" href="/favicon/apple-touch-icon-144x144.png" />
        <Link rel="apple-touch-icon-precomposed" sizes="60x60" href="/favicon/apple-touch-icon-60x60.png" />
        <Link rel="apple-touch-icon-precomposed" sizes="120x120" href="/favicon/apple-touch-icon-120x120.png" />
        <Link rel="apple-touch-icon-precomposed" sizes="76x76" href="/favicon/apple-touch-icon-76x76.png" />
        <Link rel="apple-touch-icon-precomposed" sizes="152x152" href="/favicon/apple-touch-icon-152x152.png" />
        <Link rel="icon" type_="image/png" href="/favicon/favicon-196x196.png" sizes="196x196" />
        <Link rel="icon" type_="image/png" href="/favicon/favicon-96x96.png" sizes="96x96" />
        <Link rel="icon" type_="image/png" href="/favicon/favicon-32x32.png" sizes="32x32" />
        <Link rel="icon" type_="image/png" href="/favicon/favicon-16x16.png" sizes="16x16" />
        <Link rel="icon" type_="image/png" href="/favicon/favicon-128.png" sizes="128x128" />
        <Meta name="application-name" content="Red Siren"/>
        <Meta name="msapplication-TileColor" content="#353839" />
        <Meta name="msapplication-TileImage" content="/favicon/mstile-144x144.png" />
        <Meta name="msapplication-square70x70logo" content="/favicon/mstile-70x70.png" />
        <Meta name="msapplication-square150x150logo" content="/favicon/mstile-150x150.png" />
        <Meta name="msapplication-wide310x150logo" content="/favicon/mstile-310x150.png" />
        <Meta name="msapplication-square310x310logo" content="/favicon/mstile-310x310.png" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=0" />
        <Style>
            {"@import url('https://fonts.googleapis.com/css2?family=Rosarivo:ital@0;1&display=swap');"}
        </Style>

        <Router fallback=|| {
          let mut outside_errors = Errors::default();
          outside_errors.insert_with_default_key(AppError::NotFound);
          view! {
              <ErrorTemplate outside_errors/>
          }
          .into_view()
      }>
          <main>
            <Routes>
              <Route
                path=""
                view=RedSirenCore/>
            </Routes>
          </main>
      </Router>
    }
}

pub type Core = Rc<app_core::Core<app_core::Effect, app_core::RedSiren>>;

#[derive(Clone)]
pub struct CoreContext(
    pub ReadSignal<app_core::ViewModel>,
    pub WriteSignal<Option<app_core::Event>>,
);

#[component]
fn RedSirenCore() -> impl IntoView {
    let core = core_bindings::new();
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

    let UseTimestampReturn {
        timestamp,
        pause,
        resume,
        ..
    } = use_timestamp_with_controls_and_options(UseTimestampOptions::default().immediate(false));
    let (animate_send, set_animate_send) = create_signal(None);
    let animate_cb = Callback::new(move |sender: Option<Sender<f64>>| {
        if let Some(sender) = sender {
            set_animate_send(Some(sender));
            resume();
            log::debug!("timestamp animation resumed");
        } else {
            set_animate_send(None);
            pause();
            log::debug!("timestamp animation paused");
        }
    });

    create_effect(move |last| {
        let ts = timestamp.get();

        if last != Some(ts) {
            if let Some(sender) = animate_send().as_mut() {
                sender.try_send(ts).expect("send ts");
            }
        }

        ts
    });

    let navigate = leptos_router::use_navigate();
    let navigate_cb = Callback::new(move |path| navigate(path, Default::default()));

    create_effect(move |_| {
        if let Some(ev) = event.get() {
            core_bindings::update(&core, ev, render, navigate_cb, animate_cb);
        }
    });

    let reduced_motion = use_media_query("(prefers-reduced-motion)");

    create_effect(move |_| {
        let reduced_motion = reduced_motion.get();
        set_event.set(Some(app_core::Event::Visual(
            app_core::VisualEV::SetReducedMotion(reduced_motion),
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
        set_event(Some(app_core::Event::Resize(width as f64, height as f64)));
    });

    create_effect(move |_| {
        set_event(Some(app_core::Event::SafeAreaResize(50.0, 50.0, 50.0, 50.0)));
    });

    view! {
        <div class="red-siren-core-view">
            <Intro opacity=Signal::derive(move|| view().visual.intro_opacity)/>
            <Area>
                <Object/>
            </Area>
        </div>
    }
}
