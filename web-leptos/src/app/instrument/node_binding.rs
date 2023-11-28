use js_sys::Promise;
use wasm_bindgen::JsValue;
use web_sys::{AudioContext, AudioWorkletNode};
use crate::wasm_bindgen;

#[wasm_bindgen(raw_module="/pkg/worklet/dist/lib.es.js")]
extern "C" {

    #[wasm_bindgen(extends = AudioWorkletNode)]
    pub type RedSirenNode;

    #[wasm_bindgen(constructor)]
    pub fn new(ctx: &AudioContext) -> RedSirenNode;

    #[wasm_bindgen(method)]
    pub fn init(this: &RedSirenNode) -> Promise;
    
    #[wasm_bindgen(method)]
    pub fn forward(this: &RedSirenNode, ev: &JsValue);

    #[wasm_bindgen(static_method_of = RedSirenNode)]
    pub fn addModule(ctx: &AudioContext) -> Promise;
}