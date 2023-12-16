pub use crux_core::{Core, Request};
use wasm_bindgen::prelude::wasm_bindgen;

pub use app::*;
pub use streamer::*;

mod resolve;
pub mod system;

pub mod app;

mod streamer;
cfg_if::cfg_if! {if #[cfg(feature="browser")] {
    mod instance;
    pub use instance::*;
}}

#[wasm_bindgen]
pub fn au_log_init() {
    #[allow(unused_variables)]
    let lvl = log::LevelFilter::Debug;

    #[cfg(feature = "browser")]
    {
        _ = console_log::init_with_level(lvl.to_level().unwrap_or(log::Level::Warn));
        console_error_panic_hook::set_once();
    }

    #[cfg(feature = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(lvl)
            .with_tag("red_siren::shared"),
    );

    #[cfg(feature = "ios")]
    match oslog::OsLogger::new("com.anvlkv.RedSiren.AUCore")
        .level_filter(lvl)
        .init()
    {
        Ok(_) => {}
        Err(e) => {
            log::error!("already initialized: {e:?}");
        }
    }

    log::info!("init logging")
}
