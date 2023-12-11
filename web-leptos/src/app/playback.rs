use std::{cell::RefCell, rc::Rc};

use js_sys::{Promise, Uint8Array};
use leptos::*;
use shared::play;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[derive(Clone)]
pub struct Playback(Rc<RefCell<PlaybackJs>>);

impl Playback {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(PlaybackJs::new())))
    }

    pub async fn request(&self, op: play::PlayOperation) -> play::PlayOperationOutput {
        log::trace!("playback request {op:?}");
        let data = Self::js_value_forwarded_event(op);
        let promise = self.0.borrow().request(&data);

        JsFuture::from(promise)
            .await
            .map(|op| Self::from_forwarded_effect(op))
            .expect("bridging error")
    }

    fn js_value_forwarded_event(event: play::PlayOperation) -> JsValue {
        let bin = bincode::serialize(&event).expect("event serialization err");
        let data = Uint8Array::from(bin.as_slice());
        data.into()
    }

    fn from_forwarded_effect(result: JsValue) -> play::PlayOperationOutput {
        log::trace!("playback result {result:?}");
        let data = Uint8Array::from(result);
        let mut dst = (0..data.length()).map(|_| 0 as u8).collect::<Vec<_>>();
        data.copy_to(dst.as_mut_slice());
        bincode::deserialize::<play::PlayOperationOutput>(dst.as_slice())
            .expect("effect deserialization err")
    }
}

#[wasm_bindgen(raw_module = "/worklet/lib.es.js")]
extern "C" {

    #[derive(Clone)]
    #[wasm_bindgen(js_name = PlaybackBridge)]
    pub type PlaybackJs;

    #[wasm_bindgen(constructor, js_class = "PlaybackBridge")]
    pub fn new() -> PlaybackJs;

    #[wasm_bindgen(method, js_class = "PlaybackBridge")]
    pub fn request(this: &PlaybackJs, req: &JsValue) -> Promise;

    #[wasm_bindgen(structural, method, getter, js_name = callHost, js_class = "PlaybackBridge")]
    pub fn call_host_block(this: &PlaybackJs) -> Option<::js_sys::Function>;

    #[wasm_bindgen(structural, method, setter, js_name = callHost, js_class = "PlaybackBridge")]
    pub fn set_call_host_block(this: &PlaybackJs, val: Option<&::js_sys::Function>);
}
