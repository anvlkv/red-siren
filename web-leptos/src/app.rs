mod core;
mod intro;

use std::rc::Rc;

use crate::error_template::{AppError, ErrorTemplate};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use leptos_use::{use_event_listener, use_window};
use shared::{instrument, Activity, Event};

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


        <Router fallback=|| {
          let mut outside_errors = Errors::default();
          outside_errors.insert_with_default_key(AppError::NotFound);
          view! {
              <ErrorTemplate outside_errors/>
          }
          .into_view()
      }>
          <main>
              <RedSirenRoutes/>
          </main>
      </Router>
    }
}

#[component]
fn RedSirenRoutes() -> impl IntoView {
    let core = core::new();
    let view_rw_signal = create_rw_signal(core.view());
    let view = view_rw_signal.read_only();
    let render = view_rw_signal.write_only();

    let (event, set_event) = create_signal(Event::None);
    create_effect(move |_| {
        core::update(&core, event.get(), render);
    });

    let navigate = leptos_router::use_navigate();

    create_effect(move |_| {
        let path = match view.get().activity {
            Activity::Intro => "/",
            Activity::Tune => "/tune",
            Activity::Play => "/play",
            Activity::Listen => "/listen",
        };

        navigate(path, Default::default())
    });

    let (config, set_config) = create_signal(instrument::Config::default());
    let (size, set_size) = create_signal((0, 0));
    let window = use_window();
    use_event_listener(window.clone(), leptos::ev::resize, move |_| {
        let body = window.document().body().unwrap();
        set_size.set((body.client_width(), body.client_height()));
    });

    let window = use_window();
    create_effect(move |_| {
        let body = window.document().body().unwrap();
        set_size.set((body.client_width(), body.client_height()));
    });

    let window = use_window();
    create_effect(move |_| {
        let navigator = window.navigator().unwrap();
        let (width, height) = size.get();

        let mut max_buttons = 5;

        let short_side = width.min(height);
        let long_side = width.max(height);

        if short_side < 769 {
            max_buttons = 3;
        }

        set_config.set(instrument::Config::new(
            width.try_into().unwrap(),
            height.try_into().unwrap(),
            navigator.max_touch_points() <= 1,
            max_buttons,
        ));
    });

    
    let intro_vm = create_read_slice(view_rw_signal, move |v| v.intro);
    let intro_ev = SignalSetter::map(move |ev| set_event.set(Event::IntroEvent(ev)));

    view! {
        <Routes>
            <Route path="" view=move || view! {
                <intro::IntroComponent
                    vm=intro_vm
                    ev=intro_ev
                    config=config
                />
            } />
        </Routes>
    }
}
