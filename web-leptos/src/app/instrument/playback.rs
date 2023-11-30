use leptos::*;
use leptos_use::use_window;
use shared::{
    instrument,
    Event,
};
use std::{cell::RefCell, rc::Rc};

#[allow(dead_code)]
#[derive(PartialEq, Copy, Clone)]
pub enum PlayBackState {
    Paused,
    Playing,
    Preparing,
    Error,
}

pub fn create_playback(
    playing: Selector<bool>,
    config: Signal<instrument::Config>,
    ev: SignalSetter<instrument::PlaybackEV>,
) -> (ReadSignal<PlayBackState>, WriteSignal<Option<Event>>) {
    let ev_port = create_rw_signal::<Option<Event>>(None);
    let read_ev = ev_port.read_only();
    let ev_port_input = ev_port.write_only();
    let (playback_state, set_playback_state) = create_signal(PlayBackState::Paused);

    log::debug!("create playback");

    #[cfg(feature = "browser")]
    {
        use super::node_binding::RedSirenNode;
        use js_sys::{Array, Promise, Uint8Array};
        use wasm_bindgen::{closure::Closure, JsValue};
        use web_sys::{AudioContext, AudioContextOptions, MediaStream, MediaStreamConstraints};

        fn to_forwarded_event(event: Event) -> JsValue {
            let bin = bincode::serialize(&event).expect("event serialization err");
            let data = Uint8Array::from(bin.as_slice());
            data.into()
        }

        let running_ctx: Rc<RefCell<Option<AudioContext>>> = Rc::new(RefCell::new(None));
        let running_stream: Rc<RefCell<Option<MediaStream>>> = Rc::new(RefCell::new(None));
        let running_rs_node: Rc<RefCell<Option<RedSirenNode>>> = Rc::new(RefCell::new(None));
        let window = use_window();

        let rs_node = running_rs_node.clone();
        create_effect(move |_| {
            if let Some(ev) = read_ev.get() {
                if let PlayBackState::Playing = playback_state() {
                    if let Some(node) = rs_node.borrow().as_ref() {
                        let msg = to_forwarded_event(ev);
                        node.forward(&msg);
                    }
                }
            }
        });

        let rs_node = running_rs_node.clone();
        create_effect(move |_| {
            let config = config.get();
            if let PlayBackState::Playing = playback_state() {
                if let Some(node) = rs_node.borrow().as_ref() {
                    let msg = to_forwarded_event(Event::InstrumentEvent(
                        instrument::InstrumentEV::CreateWithConfig(config),
                    ));
                    node.forward(&msg);
                }
            }
        });

        let rs_node = running_rs_node.clone();
        let audio_ctx = running_ctx.clone();
        let media_stream = running_stream.clone();
        let on_ready = Closure::new(move |val| {
            let on_ready_config = config.get_untracked();
            let val = Array::from(&val);
            let input = val.get(0);
            let _ = val.get(1);

            let ctx = audio_ctx.borrow();
            let ctx = ctx.as_ref().unwrap();
            let rs_node = rs_node.borrow();
            let rs_node = rs_node.as_ref().unwrap();

            let stream = MediaStream::from(input);

            let msg = to_forwarded_event(Event::InstrumentEvent(
                instrument::InstrumentEV::CreateWithConfig(on_ready_config),
            ));
            rs_node.forward(&msg);
            let msg = to_forwarded_event(Event::InstrumentEvent(
                instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(true)),
            ));
            rs_node.forward(&msg);

            let analyser = ctx.create_analyser().expect("analyser node");
            let src = ctx
                .create_media_stream_source(&stream)
                .expect("create source");

            let _ = src
                .connect_with_audio_node(&rs_node)
                .expect("stream to node")
                .connect_with_audio_node(&analyser)
                .expect("node to analyser")
                .connect_with_audio_node(&ctx.destination())
                .expect("to destination");

            let _ = ctx.resume().expect("resume play state");
            set_playback_state(PlayBackState::Playing);
            let _ = media_stream.borrow_mut().insert(stream);
            log::info!("playing");
        });

        let on_err_load = Closure::new(move |e| {
            log::error!("load failed {e:?}");
            let _ = set_playback_state(PlayBackState::Error);
        });

        let audio_ctx = running_ctx.clone();
        let on_loaded_rs_node = running_rs_node.clone();
        let on_loaded = Closure::new(move |_| {
            let ctx = audio_ctx.borrow();
            let ctx = ctx.as_ref().unwrap();

            log::debug!("create worklet");
            let rs_node = RedSirenNode::new(&ctx);

            log::debug!("requesting media");
            let media = window
                .navigator()
                .expect("navigator")
                .media_devices()
                .expect("media devices");

            log::debug!("requesting input");
            let input = media
                .get_user_media_with_constraints(MediaStreamConstraints::new().audio(&true.into()))
                .expect("get media");

            log::debug!("init node");
            let init = rs_node.init();

            log::debug!("persist state");
            let all = Array::from_iter(vec![input, init]);
            let _ = Promise::all(&all).then(&on_ready).catch(&on_err_load);

            let _ = on_loaded_rs_node.borrow_mut().insert(rs_node);
            log::debug!("queued");
        });

        let on_err_init = Closure::new(move |e| {
            log::error!("init failed {e:?}");
            let _ = set_playback_state(PlayBackState::Error);
        });

        let on_err_resume = Closure::new(move |e| {
            log::error!("resume failed {e:?}");
            let _ = set_playback_state(PlayBackState::Error);
        });

        let rs_node = running_rs_node.clone();
        let on_resume = Closure::new(move |_| {
            if let Some(node) = rs_node.borrow().as_ref() {
                let msg = to_forwarded_event(Event::InstrumentEvent(
                    instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(true)),
                ));
                node.forward(&msg);
            }
            let _ = set_playback_state(PlayBackState::Playing);
            log::info!("resumed");
        });

        let on_err_suspend = Closure::new(move |e| {
            log::error!("suspend failed {e:?}");
            let _ = set_playback_state(PlayBackState::Error);
        });

        let on_suspend = Closure::new(move |_| {
            let _ = set_playback_state(PlayBackState::Paused);
            log::info!("suspended");
        });

        let sample_rate = create_memo(move |_| config().sample_rate_hz);

        let rs_node = running_rs_node.clone();
        let audio_ctx = running_ctx.clone();
        // let media_stream = running_stream.clone();
        create_effect(move |_| {
            let mut effect_ctx = audio_ctx.borrow_mut();
            if playing.selected(true) {
                if let Some(ctx) = effect_ctx.as_ref() {
                    let _ = ctx
                        .resume()
                        .expect("resuming")
                        .then(&on_resume)
                        .catch(&on_err_resume);
                } else {
                    set_playback_state(PlayBackState::Preparing);
                    let mut options = AudioContextOptions::new();
                    let options = options
                        .sample_rate(sample_rate.get() as f32)
                        .latency_hint(&"balanced".into());
                    let new_ctx = AudioContext::new_with_context_options(options.as_ref())
                        .expect("audio context");

                    let _ = effect_ctx.insert(new_ctx);

                    let _ = RedSirenNode::addModule(effect_ctx.as_ref().unwrap())
                        .then(&on_loaded)
                        .catch(&on_err_init);

                    log::debug!("loading");
                }
            } else {
                if let Some(ctx) = effect_ctx.as_ref() {
                    if let Some(node) = rs_node.borrow().as_ref() {
                        let msg = to_forwarded_event(Event::InstrumentEvent(
                            instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(false)),
                        ));
                        node.forward(&msg);
                    }
                    let _ = ctx
                        .suspend()
                        .expect("suspending")
                        .then(&on_suspend)
                        .catch(&on_err_suspend);
                }
            }
        });
    }

    (playback_state, ev_port_input)
}
