pub mod geometry;

pub use crux_core::{Core, Request};
pub use crux_http as http;
pub use crux_kv as key_value;

pub mod app;
pub use app::*;

cfg_if::cfg_if!{ if #[cfg(not(any(feature="worklet", feature="browser")))]{
    mod instance;
    pub use instance::*;
} else if #[cfg(feature="browser")]{
    pub fn log_init() {
        let lvl = log::LevelFilter::Debug;
        
        _ = console_log::init_with_level(lvl.to_level().unwrap_or(log::Level::Warn));
        console_error_panic_hook::set_once();
    }
}}
