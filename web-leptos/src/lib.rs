use cfg_if::cfg_if;

pub mod app;
pub mod error_template;
mod util;

cfg_if! { if #[cfg(feature = "hydrate")] {
    use leptos::*;
    use wasm_bindgen::prelude::wasm_bindgen;
    use crate::app::*;

    #[wasm_bindgen]
    pub fn hydrate() {
        #[cfg(feature = "browser")]
        app_core::log_init();

        leptos::mount_to_body(RootComponent);
    }
}}
