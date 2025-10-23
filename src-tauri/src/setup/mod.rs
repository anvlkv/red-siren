mod commands;
#[cfg(target_os = "macos")]
mod setup_mac_window;

use common::error::{AppError, Result, SetupError};
use parking_lot::Mutex;
use serde_json::Value;
use tauri::{App, Manager};
use tauri_plugin_safe_area_insets_css::SafeAreaInsetsCssExt;
use tauri_plugin_store::StoreExt;
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
    let store = app.store(SETUP_STORE_NAME).map_err(|e| {
        AppError::Setup(SetupError::StoreSetupErr {
            message: e.to_string(),
        })
    })?;

    let override_dark = store
        .get(DARK_OVERRIDE_KEY)
        .and_then(|v: Value| v.as_bool());

    let mut main_window = app
        .get_webview_window("main")
        .ok_or(SetupError::MainWindowMissing)?;

    let size = main_window.inner_size().map_err(SetupError::window_query)?;

    let state_dark_mode = {
        // Set background color only when building for macOS
        #[cfg(target_os = "macos")]
        {
            setup_mac_window::setup(&mut main_window, override_dark)
                .map_err(SetupError::appearance)?
        }
        #[cfg(not(target_os = "macos"))]
        {
            override_dark.unwrap_or(false)
        }
    };

    let safe_area_insets = app.safe_area_insets_css();

    // Initialize window state
    let initial_state = Window {
        width: size.width as f64,
        height: size.height as f64,
        dark: state_dark_mode,
        override_dark,
        ui_safe_area: common::safe_area::SafeArea {
            top: safe_area_insets
                .get_top_inset()
                .map_err(|e| AppError::Tauri(e.to_string()))?
                .inset as f32,
            right: 0.0,
            bottom: safe_area_insets
                .get_bottom_inset()
                .map_err(|e| AppError::Tauri(e.to_string()))?
                .inset as f32,
            left: 0.0,
        },
        system_safe_area: common::safe_area::SafeArea::default(),
    };

    app.manage(Mutex::new(initial_state));

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    main_window
        .restore_state(
            tauri_plugin_window_state::StateFlags::SIZE
                & tauri_plugin_window_state::StateFlags::POSITION,
        )
        .map_err(|e| SetupError::window_state_op("restore_state", e))?;

    Ok(())
}
