use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{sync::{Mutex, MutexGuard}};

/// State to track setup completion
pub struct SetupState {
    pub gui_ready: bool,
    pub backend_ready: bool,
}

#[tauri::command]
pub async fn health_on_gui_ready(
    app: AppHandle,
    state: State<'_, Mutex<SetupState>>,
) -> Result<(), String> {
    log::info!("GUI ready signal received");

    // Lock the state and mark GUI as ready
    let mut state_lock = state.lock().await;
    state_lock.gui_ready = true;

    maybe_toggle_windows(&state_lock, &app)?;

    Ok(())
}

// Async function to simulate backend setup tasks
pub async fn perform_backend_setup(app: AppHandle, state: &Mutex<SetupState>) -> Result<(), String> {
    log::info!("Performing backend setup tasks...");

    // Mark backend as ready
    let mut state_lock = state.lock().await;
    state_lock.backend_ready = true;

    maybe_toggle_windows(&state_lock, &app)?;

    Ok(())
}

/// Check if both GUI and backend are ready
///
/// Hide `splashscreen` window
///
/// Show `main` window
fn maybe_toggle_windows(state_lock: &MutexGuard<'_, SetupState>, app: &AppHandle) -> Result<(), String> {
    #[cfg(not(any(target_os="ios", target_os="android")))]
    if state_lock.gui_ready && state_lock.backend_ready {
        log::info!("Both GUI and backend ready, closing splashscreen and showing main window");

        // Close splashscreen and show main window
        if let Some(splash_window) = app.get_webview_window("splashscreen") {
            splash_window.close().map_err(|e| format!("Failed to close splashscreen: {}", e))?;
        }

        if let Some(main_window) = app.get_webview_window("main") {
            main_window.show().map_err(|e| format!("Failed to show main window: {}", e))?;

            main_window.set_focus().map_err(|e| format!("Failed to focus main window: {}", e))?;
        }

        app.emit(shared::events::health::APP_READY, ()).map_err(|e| e.to_string())?;

        log::debug!("Emited: {}", shared::events::health::APP_READY);
    }
    else {
        log::debug!("GUI ready: {}. Backend ready: {}", state_lock.gui_ready, state_lock.backend_ready);
    }

    Ok(())
}
