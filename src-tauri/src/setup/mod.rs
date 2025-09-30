mod commands;
#[cfg(target_os = "macos")]
mod setup_mac_window;

use shared::error::{Result, SetupError};
use tauri::{App, Manager};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use tauri_plugin_window_state::WindowExt;
use tokio::sync::Mutex;

pub use commands::*;

#[derive(Debug, Clone)]
pub struct Window {
    pub dark: bool,
    pub width: f64,
    pub height: f64,
}

pub type WindowState = Mutex<Window>;

pub fn app_setup(app: &mut App) -> Result<()> {
    let mut main_window = app
        .get_webview_window("main")
        .ok_or(SetupError::MainWindowMissing)?;

    let size = main_window.inner_size().map_err(SetupError::window_query)?;

    // Initialize window state
    let initial_state = Window {
        width: size.width as f64,
        height: size.height as f64,
        dark: false,
    };

    app.manage(Mutex::new(initial_state));

    // Set background color only when building for macOS
    #[cfg(target_os = "macos")]
    {
        setup_mac_window::setup(&mut main_window).map_err(SetupError::appearance)?;
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    main_window
        .restore_state(
            tauri_plugin_window_state::StateFlags::SIZE
                & tauri_plugin_window_state::StateFlags::POSITION,
        )
        .map_err(|e| SetupError::window_state_op("restore_state", e))?;

    Ok(())
}
