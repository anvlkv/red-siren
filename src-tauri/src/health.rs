use tauri::{AppHandle, Manager, State};
use tokio::{sync::{Mutex, MutexGuard}, time::{sleep, Duration}};

// State to track setup completion
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

    // Simulate some backend initialization (e.g., database connections, file loading)
    sleep(Duration::from_secs(2)).await;
    log::info!("Backend setup tasks completed");

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
    if state_lock.gui_ready && state_lock.backend_ready {
        log::info!("Both GUI and backend ready, closing splashscreen and showing main window");

        // Close splashscreen and show main window
        if let Some(splash_window) = app.get_webview_window("splashscreen") {
            splash_window.close().map_err(|e| format!("Failed to close splashscreen: {}", e))?;
        }

        if let Some(main_window) = app.get_webview_window("main") {
            main_window.show().map_err(|e| format!("Failed to show main window: {}", e))?;
        }
    }
    else {
        log::debug!("GUI ready: {}. Backend ready: {}", state_lock.gui_ready, state_lock.backend_ready);
    }

    Ok(())
}
