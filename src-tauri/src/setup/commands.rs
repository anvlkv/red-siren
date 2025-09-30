use tauri::{AppHandle, Emitter, Manager, State};
use shared::error::{Result, SetupError};

use super::WindowState;

#[tauri::command]
pub async fn update_window_appearance(
    dark: bool,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    let mut main_window = app
        .get_webview_window("main")
        .ok_or(SetupError::MainWindowMissing)?;

    #[cfg(target_os = "macos")]
    {
        super::setup_mac_window::update_appearance(&mut main_window, dark)
            .map_err(SetupError::appearance)?;
    }

    let mut state_lock = state.lock().await;
    state_lock.dark = dark;

    app.emit(shared::events::setup::UPDATE_WINDOW_APPEARANCE, ())
        .map_err(|e| SetupError::emit(shared::events::setup::UPDATE_WINDOW_APPEARANCE, e))?;

    Ok(())
}

#[tauri::command]
pub async fn update_window_size(
    width: f64,
    height: f64,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    let mut state_lock = state.lock().await;
    state_lock.width = width;
    state_lock.height = height;

    app.emit(shared::events::setup::UPDATE_WINDOW_SIZE, ())
        .map_err(|e| SetupError::emit(shared::events::setup::UPDATE_WINDOW_SIZE, e))?;

    Ok(())
}
