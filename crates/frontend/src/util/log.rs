use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::Once;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use js_sys::{Object, Reflect};

#[wasm_bindgen]
extern "C" {
    // Call Tauri core invoke directly: invoke('plugin:log|log', { level, message, file, line, keyValues })
    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"])]
    fn invoke(cmd: &str, args: &JsValue) -> js_sys::Promise;
}

struct TauriLogger;

impl Log for TauriLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        // Keep everything enabled; set_max_level will gate the effective level.
        true
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let message = format!("{}", record.args());

        // Build fresh args object with exact fields expected by plugin:log|log
        let args_obj = Object::new();

        // level as numeric (Trace=1..Error=5)
        let level_num = match record.level() {
            Level::Trace => 1,
            Level::Debug => 2,
            Level::Info => 3,
            Level::Warn => 4,
            Level::Error => 5,
        };
        let _ = Reflect::set(
            &args_obj,
            &JsValue::from_str("level"),
            &JsValue::from_f64(level_num as f64),
        );
        let _ = Reflect::set(
            &args_obj,
            &JsValue::from_str("message"),
            &JsValue::from_str(&message),
        );

        // file and line
        if let Some(file) = record.file() {
            let _ = Reflect::set(&args_obj, &JsValue::from_str("file"), &JsValue::from_str(file));
        }
        if let Some(line) = record.line() {
            let _ = Reflect::set(&args_obj, &JsValue::from_str("line"), &JsValue::from_f64(line as f64));
        }

        // location string like "<module>@<file>:<line>" or "@<file>:<line>"
        if let (Some(file), Some(line)) = (record.file(), record.line()) {
            if let Some(module_path) = record.module_path() {
                let _ = Reflect::set(
                    &args_obj,
                    &JsValue::from_str("location"),
                    &JsValue::from_str(&format!("{}@{}:{}", module_path, file, line)),
                );
            } else {
                let _ = Reflect::set(
                    &args_obj,
                    &JsValue::from_str("location"),
                    &JsValue::from_str(&format!("@{}:{}", file, line)),
                );
            }
        } else if let Some(module_path) = record.module_path() {
            let _ = Reflect::set(
                &args_obj,
                &JsValue::from_str("location"),
                &JsValue::from_str(module_path),
            );
        }

        // key_values map
        let key_values = Object::new();
        let _ = Reflect::set(
            &key_values,
            &JsValue::from_str("target"),
            &JsValue::from_str(record.target()),
        );
        if let Some(module_path) = record.module_path() {
            let _ = Reflect::set(
                &key_values,
                &JsValue::from_str("module_path"),
                &JsValue::from_str(module_path),
            );
        }
        let _ = Reflect::set(
            &args_obj,
            &JsValue::from_str("key_values"),
            &JsValue::from(key_values),
        );

        let args = JsValue::from(args_obj);
        spawn_local(async move {
            // Fire and forget; ignore the result.
            let _ = JsFuture::from(invoke("plugin:log|log", &args)).await;
        });
    }

    fn flush(&self) {
        // Async logger; nothing to flush.
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
