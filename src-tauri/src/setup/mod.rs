mod commands;
#[cfg(target_os = "macos")]
mod setup_mac_window;

use serde_json::Value;
use shared::error::{AppError, Result, SetupError};
use tauri::{App, Manager};
use tauri_plugin_store::StoreExt;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use tauri_plugin_window_state::WindowExt;
use tokio::sync::Mutex;

pub use commands::*;

pub(super) const SETUP_STORE_NAME: &str = "setup.json";
pub(super) const DARK_OVERRIDE_KEY: &str = "dark_override";
#[derive(Debug, Clone)]
pub struct Window {
    pub dark: bool,
    pub override_dark: Option<bool>,
    pub width: f64,
    pub height: f64,
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

    // Initialize window state
    let initial_state = Window {
        width: size.width as f64,
        height: size.height as f64,
        dark: state_dark_mode,
        override_dark,
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
