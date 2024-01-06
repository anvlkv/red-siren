use cfg_if::cfg_if;

pub mod app;
pub mod error_template;
pub mod fileserv;
mod util;

cfg_if! { if #[cfg(feature = "hydrate")] {
    use leptos::*;
    use wasm_bindgen::prelude::wasm_bindgen;
    use crate::app::*;

    #[wasm_bindgen]
    pub fn hydrate() {
        #[cfg(feature = "browser")]
        app_core::log_init(log::LevelFilter::Trace);

        leptos::mount_to_body(RootComponent);
    }
}}
