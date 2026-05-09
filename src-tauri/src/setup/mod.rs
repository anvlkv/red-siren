mod commands;
#[cfg(target_os = "macos")]
mod setup_mac_window;

use crate::persistence::persistence::load_json;
use common::error::Result;
use parking_lot::Mutex;
use tauri::{App, Manager};
use tauri_plugin_safe_area_insets_css::SafeAreaInsetsCssExt;

#[cfg(not(any(target_os = "ios", target_os = "android")))]
use tauri_plugin_window_state::WindowExt; // parking_lot chosen over tokio::sync::Mutex to avoid awaiting locks and reduce deadlock risk

pub use commands::*;

pub(super) const SETUP_STORE_NAME: &str = "setup.json";
pub(super) const DARK_OVERRIDE_KEY: &str = "dark_override";
#[derive(Debug, Clone)]
pub struct Window {
    pub dark: bool,
    pub override_dark: Option<bool>,
    pub width: f64,
    pub height: f64,
    pub ui_safe_area: common::safe_area::SafeArea,
    pub system_safe_area: common::safe_area::SafeArea,
}

pub type WindowState = Mutex<Window>;

pub fn app_setup(app: &mut App) -> Result<()> {
    // Load Option<bool> directly so null or missing entries don't fail setup
    let override_dark: Option<bool> =
        match load_json::<Option<bool>>(app.handle(), SETUP_STORE_NAME, DARK_OVERRIDE_KEY) {
            Ok(v) => v.flatten(),
            Err(e) => {
                log::error!(
                    "Failed to load window appearance override from store {} key {}: {}",
                    SETUP_STORE_NAME,
                    DARK_OVERRIDE_KEY,
                    e
                );
                None
            }
        };

    // Defaults; will be overridden if main window is available
    let mut width = 800.0_f64;
    let mut height = 600.0_f64;
    let mut state_dark_mode = override_dark.unwrap_or(false);

    // Try to access main window; never early-return on errors to ensure state is managed
    let mut main_window_opt = app.get_webview_window("main");

    if let Some(ref mut main_window) = main_window_opt {
        match main_window.inner_size() {
            Ok(size) => {
                width = size.width as f64;
                height = size.height as f64;
            }
            Err(e) => {
                log::warn!(
                    "Main window size unavailable during setup, using defaults: {}",
                    e
                );
            }
        }

        // Set background color only when building for macOS
        #[cfg(target_os = "macos")]
        {
            match setup_mac_window::setup(main_window, override_dark) {
                Ok(dark) => state_dark_mode = dark,
                Err(e) => {
                    log::warn!(
                        "Mac window appearance setup failed, using override/default: {}",
                        e
                    );
                    state_dark_mode = override_dark.unwrap_or(false);
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            state_dark_mode = override_dark.unwrap_or(false);
        }
    } else {
        log::warn!("Main window not available in setup; using default size and dark mode");
    }

    let safe_area_insets = app.safe_area_insets_css();
    let top_inset = safe_area_insets
        .get_top_inset()
        .map(|v| v.inset)
        .unwrap_or_else(|e| {
            log::warn!("Failed to get top safe area inset: {}", e);
            0.0
        });
    let bottom_inset = safe_area_insets
        .get_bottom_inset()
        .map(|v| v.inset)
        .unwrap_or_else(|e| {
            log::warn!("Failed to get bottom safe area inset: {}", e);
            0.0
        });

    // Initialize window state
    let initial_state = Window {
        width,
        height,
        dark: state_dark_mode,
        override_dark,
        ui_safe_area: common::safe_area::SafeArea {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        },
        system_safe_area: common::safe_area::SafeArea {
            top: top_inset,
            right: 0.0,
            bottom: bottom_inset,
            left: 0.0,
        },
    };

    app.manage(Mutex::new(initial_state));

    // Best-effort restore and window event wiring if window exists
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if let Some(ref mut main_window) = main_window_opt {
        if let Err(e) = main_window.restore_state(
            tauri_plugin_window_state::StateFlags::SIZE
                & tauri_plugin_window_state::StateFlags::POSITION,
        ) {
            log::warn!("restore_state failed: {}", e);
        }
    }

    if let Some(main_window) = main_window_opt {
        main_window.on_window_event({
            let app_handle = app.app_handle().clone();
            move |event| {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    app_handle.exit(0);
                }
            }
        });
    }

    Ok(())
}
