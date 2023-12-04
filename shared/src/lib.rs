pub mod geometry;

use cfg_if::cfg_if;
use lazy_static::lazy_static;
use wasm_bindgen::prelude::wasm_bindgen;

use crux_core::bridge::Bridge;
pub use crux_core::{Core, Request};
pub use crux_http as http;
pub use crux_kv as key_value;
pub use crux_platform as platform;
pub use crux_time as time;

pub mod app;
pub use app::*;

pub const MAX_AUDIO_BUFFER_SIZE: usize = fundsp::MAX_BUFFER_SIZE;
// TODO see if crux already does it.

uniffi::include_scaffolding!("shared");
lazy_static! {
    static ref CORE: Bridge<Effect, RedSiren> = Bridge::new(Core::new::<RedSirenCapabilities>());
}

#[wasm_bindgen]
pub fn process_event(data: &[u8]) -> Vec<u8> {
    CORE.process_event(data)
}

#[wasm_bindgen]
pub fn handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8> {
    CORE.handle_response(uuid, data)
}

#[wasm_bindgen]
pub fn view() -> Vec<u8> {
    CORE.view()
}

#[wasm_bindgen]
pub fn log_init() {
    #[allow(unused_variables)]
    let lvl = log::LevelFilter::Debug;

    #[cfg(feature = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(lvl)
                .with_tag("red_siren::shared"),
        );
    }
    #[cfg(feature = "ios")]
    {

        cfg_if! { if #[cfg(feature="wokrlet")] {
            let lg = oslog::OsLogger::new("com.anvlkv.redsiren.RedSiren.AUExtension");
        } else {
            let lg = oslog::OsLogger::new("com.anvlkv.RedSiren.Core");
        }}

        lg.level_filter(lvl).init().unwrap();
    }
    #[cfg(feature = "browser")]
    {
        _ = console_log::init_with_level(lvl.to_level().unwrap_or(log::Level::Warn));
        console_error_panic_hook::set_once();
    }

    log::debug!("init logging")
}
