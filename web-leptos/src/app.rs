use cfg_if::cfg_if;
use futures::channel::mpsc::Sender;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use leptos_use::{
    use_event_listener, use_timestamp_with_controls_and_options, use_window, UseTimestampOptions,
    UseTimestampReturn,
};


use crate::{
    error_template::{AppError, ErrorTemplate},
    util::use_dpi,
};

mod about;
mod core_bindings;
mod instrument;
mod intro;
mod red_card;
mod menu;
mod tuner;

cfg_if! { if #[cfg(feature="browser")]{
    mod playback;
} else {
    mod playback {

        #[derive(Clone)]
        pub struct Playback;

        impl Playback {
            pub fn new() -> Self {
                Self
            }

            pub fn on_capture(&self, _: leptos::WriteSignal<app_core::Event>){
                unimplemented!()
            }
        }
    }
}}

pub fn provide_playback() {
    let (r_op, _) = create_signal::<playback::Playback>(playback::Playback::new());
    provide_context(r_op)
}

#[component]
pub fn RootComponent() -> impl IntoView {
    provide_meta_context();

    provide_playback();

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
          <main class="bg-red dark:bg-black text-black dark:text-red font-serif">
              <RedSirenCore/>
          </main>
      </Router>
    }
}

#[component]
fn RedSirenCore() -> impl IntoView {
    let core = core_bindings::new();
    let view_rw_signal = create_rw_signal(core.view());
    let render = view_rw_signal.write_only();
    let playback = use_context::<ReadSignal<playback::Playback>>().unwrap();
    let (event, set_event) = create_signal(app_core::Event::Start);

    create_effect(move|_| {
        let pb = playback();
        pb.on_capture(set_event);
    });

    let navigate = leptos_router::use_navigate();
    let navigate_cb = Callback::new(move |path| navigate(path, Default::default()));

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

    create_effect(move |_| {
        core_bindings::update(
            &core,
            event.get(),
            render,
            playback.get(),
            navigate_cb,
            animate_cb,
        );
    });

    let location = leptos_router::use_location();

    create_effect(move |_| {
        let pathname = (location.pathname)();
        log::debug!("browser or user activated pathname: {pathname}");
        match pathname.as_str() {
            "/tune" => set_event(app_core::Event::ReflectActivity(app_core::Activity::Tune)),
            "/play" => set_event(app_core::Event::ReflectActivity(app_core::Activity::Play)),
            "/listen" => set_event(app_core::Event::ReflectActivity(app_core::Activity::Listen)),
            "/about" => set_event(app_core::Event::ReflectActivity(app_core::Activity::About)),
            _ => set_event(app_core::Event::ReflectActivity(app_core::Activity::Intro)),
        }
    });

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

    let dpi = use_dpi(vec![120, 160, 240, 320, 480, 640]);
    create_effect(move |_| {
        let (width, height) = size.get();
        let dpi = dpi.get() as f64;

        set_event(app_core::Event::CreateConfigAndConfigureApp {
            width: width as f64,
            height: height as f64,
            dpi,
            safe_areas: [50.0, 50.0, 50.0, 50.0],
        })
    });

    let intro_vm = create_read_slice(view_rw_signal, move |v| v.intro.clone());
    let intro_ev = SignalSetter::map(move |ev| set_event.set(app_core::Event::IntroEvent(ev)));
    let instrument_vm = create_read_slice(view_rw_signal, move |v| v.instrument.clone());
    let instrument_ev = SignalSetter::map(move |ev| set_event.set(app_core::Event::InstrumentEvent(ev)));
    let tuner_vm = create_read_slice(view_rw_signal, move |v| v.tuner.clone());
    let tuner_ev = SignalSetter::map(move |ev| set_event.set(app_core::Event::TunerEvent(ev)));

    let view_box = Signal::derive(move || {
        let vb = view_rw_signal.get().view_box;
        format!(
            "{} {} {} {}",
            vb.top_left().x,
            vb.top_left().y,
            vb.bottom_right().x,
            vb.bottom_right().y
        )
    });

    provide_context(set_event);

    view! {
        <Routes>
            <Route path="" view=move || view! {
                <intro::IntroComponent
                    vm=intro_vm
                    ev=intro_ev
                />
            } />
            <Route path="about" view=move || view! {
                <about::AboutComponent
                    vm=intro_vm
                />
            } />
            <Route path="play" view=move || view! {
                <instrument::InstrumentComponent
                    view_box=view_box
                    vm=instrument_vm
                    ev=instrument_ev
                />
            } />
            <Route path="tune" view=move || view! {
                <tuner::TunerComponent
                    view_box=view_box
                    vm=tuner_vm
                    ev=tuner_ev
                />
            } />
        </Routes>
    }
}
