pub use crux_core::{Core, Request};
pub use crux_http as http;
pub use crux_kv as key_value;

pub use app::*;

pub mod geometry;

pub mod app;
cfg_if::cfg_if! { if #[cfg(feature="instance")]{
    mod instance;
    pub use instance::*;
} else if #[cfg(feature="browser")]{
    pub fn log_init() {
        let lvl = log::Level::Debug;

        _ = console_log::init_with_level(lvl);
        console_error_panic_hook::set_once();

        log::info!("init logging")
    }
}}
