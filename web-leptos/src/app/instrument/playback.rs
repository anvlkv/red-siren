use crate::kv::KVContext;
use leptos::*;
use leptos_use::use_window;
use shared::instrument::{self, INPUT_STREAM_KV};
use std::{cell::RefCell, rc::Rc};

#[derive(PartialEq, Copy, Clone)]
pub enum PlayBackState {
    Paused,
    Playing,
    Preparing,
    Error,
}

pub fn create_playback(
    playing: Signal<bool>,
    config: Signal<instrument::Config>,
    ev: SignalSetter<instrument::PlaybackEV>,
) -> ReadSignal<PlayBackState> {
    let kv_ctx = use_context::<KVContext>().unwrap();
    let kv_ref = kv_ctx.get();

    let (playback_state, set_playback_state) = create_signal(PlayBackState::Paused);

    #[cfg(feature = "browser")]
    {
        use wasm_bindgen::{closure::Closure, JsCast, JsValue};
        use web_sys::{AudioContext, MediaStream, MediaStreamConstraints};

        let running_ctx: Rc<RefCell<Option<AudioContext>>> = Rc::new(RefCell::new(None));
        let window = use_window();

        let audio_ctx = running_ctx.clone();

        

        let on_get_input = Closure::new(move |val| {
            let stream = MediaStream::from(val);
            let mut ctx = audio_ctx.borrow_mut();
            let ctx = ctx.as_mut().unwrap();
            let analyser = ctx.create_analyser().expect("analyser node");
            /* 
            *   TODO: so i stuck here.. i would hate to use animation frame for audio, the listener perhaps too
            *   couldn't get the worklet example to work. nor it works in mobile browser see https://rustwasm.github.io/wasm-bindgen/examples/wasm-audio-worklet.html
            *   see https://github.com/rustwasm/wasm-bindgen/issues/210
            *   I will consider rebuilding the DSP here in web audio this will require some parallel implementation of playback
            */ 
        });

        let on_err_input = Closure::new(move |_| {
            log::error!("can't start input stream");
            let _ = set_playback_state(PlayBackState::Error);
        });

        create_effect(move |_| {
            if let Some(prev) = running_ctx.borrow_mut().take() {
                let _ = prev.close().expect("close previous ctx");
            }

            if playing() {
                set_playback_state(PlayBackState::Preparing);
                let audio_ctx = AudioContext::new().expect("audio context");

                let media = window
                    .navigator()
                    .expect("navigator")
                    .media_devices()
                    .expect("media devices");

                let input = media
                    .get_user_media_with_constraints(
                        MediaStreamConstraints::new().audio(&true.into()),
                    )
                    .expect("get media");

                let _ = input.then(&on_get_input).catch(&on_err_input);


                let _ = audio_ctx.resume().expect("resume play state");

                let _ = running_ctx.borrow_mut().insert(audio_ctx);
            } else {
                set_playback_state(PlayBackState::Paused);
            }
        });
    }

    playback_state
}
