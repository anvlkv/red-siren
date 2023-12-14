pub use crux_core::{Core, Request};

pub use app::*;

mod resolve;
pub mod system;

pub mod app;
cfg_if::cfg_if! {if #[cfg(feature="browser")] {
    mod instance;
    pub use instance::*;
} else {
    mod streamer;
    pub use streamer::*;
}}
