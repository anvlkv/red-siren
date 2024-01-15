use std::{cell::RefCell, rc::Rc};

use app_core::{play, Event};
use js_sys::{Promise, Uint8Array};
use leptos::{WriteSignal, SignalSet, request_animation_frame};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[derive(Clone)]
pub struct Playback(
    Rc<RefCell<PlaybackBridgeJs>>,
    Rc<RefCell<Closure<dyn FnMut(JsValue)>>>,
);

impl Playback {
    pub fn new() -> Self {
        Self(
            Rc::new(RefCell::new(PlaybackBridgeJs::new())),
            Rc::new(RefCell::new(Closure::wrap(Box::new(move |_: JsValue| {
                unimplemented!("no capture callback registered")
            })
                as Box<dyn FnMut(JsValue)>))),
        )
    }

    pub async fn request(&mut self, op: play::PlayOperation) -> play::PlayOperationOutput {
        log::trace!("playback request {op:?}");

        let data = Self::js_value_forwarded_event(op);

        let promise = self.0.borrow().request(&data);

        JsFuture::from(promise)
            .await
            .map(|op| Self::play_op_from_forwarded_effect(op))
            .expect("bridging error")
    }

    pub fn on_capture(&self, set_ev: WriteSignal<Event>) {
        let cb = Closure::wrap(Box::new(move |d: JsValue| {
            let capture = Self::capture_from_forwarded_effect(d);
            request_animation_frame(move|| {
                set_ev.set(Event::Capture(capture));
            })
        }) as Box<dyn FnMut(JsValue)>);
        *self.1.borrow_mut() = cb;

        self.0
            .borrow()
            .set_on_capture(self.1.borrow().as_ref().unchecked_ref())
    }

    fn js_value_forwarded_event(event: play::PlayOperation) -> JsValue {
        let bin = bincode::serialize(&event).expect("event serialization err");
        let data = Uint8Array::from(bin.as_slice());
        data.into()
    }

    fn play_op_from_forwarded_effect(result: JsValue) -> play::PlayOperationOutput {
        let data = Uint8Array::from(result);
        let mut dst = (0..data.length()).map(|_| 0 as u8).collect::<Vec<_>>();
        data.copy_to(dst.as_mut_slice());
        let result = bincode::deserialize::<play::PlayOperationOutput>(dst.as_slice()).expect("effect deserialization err");

        // log::trace!("playback result {result:?}");

        result
    }
    
    fn capture_from_forwarded_effect(result: JsValue) -> play::CaptureOutput {
        let data = Uint8Array::from(result);
        let mut dst = (0..data.length()).map(|_| 0 as u8).collect::<Vec<_>>();
        data.copy_to(dst.as_mut_slice());
        let result = bincode::deserialize::<play::CaptureOutput>(dst.as_slice()).expect("effect deserialization err");

        // log::trace!("playback result {result:?}");

        result
    }
}

#[wasm_bindgen(raw_module = "/worklet/lib.es.js")]
extern "C" {

    #[derive(Clone)]
    #[wasm_bindgen(js_name = PlaybackBridge)]
    pub type PlaybackBridgeJs;

    #[wasm_bindgen(constructor, js_class = "PlaybackBridge")]
    pub fn new() -> PlaybackBridgeJs;

    #[wasm_bindgen(method, js_class = "PlaybackBridge")]
    pub fn request(this: &PlaybackBridgeJs, req: &JsValue) -> Promise;

    #[wasm_bindgen(method, structural, setter, js_class = "PlaybackBridge")]
    pub fn set_on_capture(this: &PlaybackBridgeJs, val: &js_sys::Function);
}
