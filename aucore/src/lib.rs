mod resolve;
pub mod system;

pub use crux_core::{Core, Request};

pub mod app;
pub use app::*;

cfg_if::cfg_if! {if #[cfg(feature="browser")] {
    mod instance;
    pub use instance::*;
}}
