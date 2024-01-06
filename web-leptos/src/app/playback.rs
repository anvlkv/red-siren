use std::{borrow::BorrowMut, cell::RefCell, rc::Rc};

use app_core::play;
use futures::channel::mpsc::{unbounded, UnboundedReceiver};
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

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
                unimplemented!("no resolve")
            })
                as Box<dyn FnMut(JsValue)>))),
        )
    }

    pub async fn request(
        &mut self,
        op: play::PlayOperation,
    ) -> UnboundedReceiver<play::PlayOperationOutput> {
        log::trace!("playback request {op:?}");

        let (sx, rx) = unbounded::<play::PlayOperationOutput>();

        _ = self
            .1
            .borrow_mut()
            .replace(Closure::wrap(Box::new(move |d: JsValue| {
                let data = Self::from_forwarded_effect(d);
                sx.unbounded_send(data).expect("send data");
            }) as Box<dyn FnMut(JsValue)>));


        let data = Self::js_value_forwarded_event(op);
        
        self.0.borrow().request(&data, self.1.borrow().as_ref().unchecked_ref());

        rx
    }

    fn js_value_forwarded_event(event: play::PlayOperation) -> JsValue {
        let bin = bincode::serialize(&event).expect("event serialization err");
        let data = Uint8Array::from(bin.as_slice());
        data.into()
    }

    fn from_forwarded_effect(result: JsValue) -> play::PlayOperationOutput {
        let data = Uint8Array::from(result);
        let mut dst = (0..data.length()).map(|_| 0 as u8).collect::<Vec<_>>();
        data.copy_to(dst.as_mut_slice());
        let result = bincode::deserialize::<play::PlayOperationOutput>(dst.as_slice())
            .expect("effect deserialization err");

        log::trace!("playback result {result:?}");

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
    pub fn request(this: &PlaybackBridgeJs, req: &JsValue, sender: &::js_sys::Function);
}
