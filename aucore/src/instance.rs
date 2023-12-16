use crux_core::bridge::Bridge;
use lazy_static::lazy_static;
use wasm_bindgen::prelude::wasm_bindgen;

use super::*;

lazy_static! {
    static ref AU_CORE: Bridge<Effect, RedSirenAU> =
        Bridge::new(Core::new::<RedSirenAUCapabilities>());
}

#[wasm_bindgen]
pub fn au_process_event(data: &[u8]) -> Vec<u8> {
    AU_CORE.process_event(data)
}

#[wasm_bindgen]
pub fn au_handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8> {
    AU_CORE.handle_response(uuid, data)
}

#[wasm_bindgen]
pub fn au_view() -> Vec<u8> {
    AU_CORE.view()
}
