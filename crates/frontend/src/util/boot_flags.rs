use std::sync::OnceLock;

use js_sys::Reflect;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone, Copy, Default)]
pub struct BootFlags {
    pub devtools: bool,
    pub secondary: bool,
}

static BOOT_FLAGS: OnceLock<BootFlags> = OnceLock::new();

/// Returns the boot flags captured before the frontend mounts.
///
/// Values are injected by the Tauri backend through `window.__RED_SIREN_FLAGS__`.
pub fn boot_flags() -> BootFlags {
    *BOOT_FLAGS.get_or_init(read_boot_flags)
}

fn read_boot_flags() -> BootFlags {
    let window = match web_sys::window() {
        Some(win) => win,
        None => return BootFlags::default(),
    };

    let flags = match Reflect::get(&window, &JsValue::from_str("__RED_SIREN_FLAGS__")) {
        Ok(val) => val,
        Err(_) => return BootFlags::default(),
    };

    BootFlags {
        devtools: read_bool(&flags, "devtools"),
        secondary: read_bool(&flags, "secondary"),
    }
}

fn read_bool(flags: &JsValue, key: &str) -> bool {
    Reflect::get(flags, &JsValue::from_str(key))
        .ok()
        .and_then(|val| val.as_bool())
        .unwrap_or(false)
}
