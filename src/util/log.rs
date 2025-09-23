use log::{Log, Metadata, Record, Level, LevelFilter};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use std::sync::Once;


#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["__TAURI_PLUGIN_LOG__"])]
    async fn trace(message: &str);

    #[wasm_bindgen(js_namespace = ["__TAURI_PLUGIN_LOG__"])]
    async fn debug(message: &str);

    #[wasm_bindgen(js_namespace = ["__TAURI_PLUGIN_LOG__"])]
    async fn info(message: &str);

    #[wasm_bindgen(js_namespace = ["__TAURI_PLUGIN_LOG__"])]
    async fn warn(message: &str);

    #[wasm_bindgen(js_namespace = ["__TAURI_PLUGIN_LOG__"])]
    async fn error(message: &str);
}

struct TauriLogger;

impl Log for TauriLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true // Enable all logging levels
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let message = format!("{}", record.args());

            // Use spawn_local since the JS functions are async
            match record.level() {
                Level::Error => {
                    spawn_local(async move {
                        error(&message).await;
                    });
                }
                Level::Warn => {
                    spawn_local(async move {
                        warn(&message).await;
                    });
                }
                Level::Info => {
                    spawn_local(async move {
                        info(&message).await;
                    });
                }
                Level::Debug => {
                    spawn_local(async move {
                        debug(&message).await;
                    });
                }
                Level::Trace => {
                    spawn_local(async move {
                        trace(&message).await;
                    });
                }
            }
        }
    }

    fn flush(&self) {
        // Nothing to flush for async operations
    }
}

static TAURI_LOGGER: TauriLogger = TauriLogger;


static INIT: Once = Once::new();

pub fn init_tauri_logger() {
    INIT.call_once(|| {
        log::set_logger(&TAURI_LOGGER)
            .map(|()| log::set_max_level(LevelFilter::Trace))
            .expect("Failed to initialize Tauri logger");
    });
}
