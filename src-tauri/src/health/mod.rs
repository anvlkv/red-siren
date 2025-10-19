// NOTE: mic permission functions moved to audio_worklet layer
// use audio_system::rt::{check_mic_permission, supports_mic};
use serde_json::Value;
use tauri::{App, AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use parking_lot::{Mutex, MutexGuard}; // switched from tokio::sync::Mutex to parking_lot for non-async, faster locking
use common::error::HealthError;

#[derive(Default, Debug, Clone, Copy)]
/// State to track setup completion
pub struct SetupState {
    pub gui_ready: bool,
    pub mic_permission: Option<bool>,
}

const HEALTH_STORE_NAME:&str = "health.json";
const MIC_PEMISSION_KEY:&str = "mic_permission";

pub type HealthSetupState = Mutex<SetupState>;

pub fn setup(app: &mut App) -> tauri_plugin_store::Result<()> {
    let store = app.store(HEALTH_STORE_NAME)?;

    let mic_permission = store.get(MIC_PEMISSION_KEY).and_then(|v: Value| v.as_bool());

    let initial_state = SetupState {
        mic_permission,
        ..Default::default()
    };

    // Emit initial setup state
    app.emit(
        common::events::health::SETUP_STATE,
        common::commands::health::SetupStatePayload {
            gui_ready: initial_state.gui_ready,
            mic_permission: initial_state.mic_permission,
        },
    ).ok(); // Ignore error during setup

    app.manage(Mutex::new(initial_state));

    Ok(())
}

#[tauri::command]
pub async fn health_grant_mic_premission(
    app: AppHandle,
    state: State<'_, Mutex<SetupState>>,
    prompt: bool,
) -> common::error::Result<bool> {
    // If prompting, run full check and propagate any HealthError (mapped automatically into AppError).
    let check_result = if prompt {
        // TODO: Implement mic permission check via audio_worklet commands
        // For now, return false as placeholder
        false
    } else {
        false
    };

    let mut health_state = state.lock();

    health_state.mic_permission = Some(check_result);

    // Emit the updated setup state
    app.emit(
        common::events::health::SETUP_STATE,
        common::commands::health::SetupStatePayload {
            gui_ready: health_state.gui_ready,
            mic_permission: health_state.mic_permission,
        },
    )
    .map_err(|e| HealthError::Emit {
        event: common::events::health::SETUP_STATE.to_string(),
        message: e.to_string(),
    })?;

    let store = app.store(HEALTH_STORE_NAME).unwrap();

    store.set(MIC_PEMISSION_KEY, check_result);

    Ok(check_result)
}

#[tauri::command]
pub async fn health_on_gui_ready(
    app: AppHandle,
    state: State<'_, Mutex<SetupState>>,
) -> common::error::Result<()> {
    log::info!("GUI ready signal received");

    // Lock the state and mark GUI as ready
    let mut state_lock = state.lock();
    state_lock.gui_ready = true;

    // Emit the updated setup state
    app.emit(
        common::events::health::SETUP_STATE,
        common::commands::health::SetupStatePayload {
            gui_ready: state_lock.gui_ready,
            mic_permission: state_lock.mic_permission,
        },
    )
    .map_err(|e| HealthError::Emit {
        event: common::events::health::SETUP_STATE.to_string(),
        message: e.to_string(),
    })?;

    maybe_toggle_windows(&state_lock, &app)?;

    Ok(())
}

#[tauri::command]
pub async fn health_setup_state(
    app: AppHandle,
    state: State<'_, Mutex<SetupState>>,
) -> common::error::Result<common::commands::health::SetupStatePayload> {
    let state_lock = state.lock();

    let payload = common::commands::health::SetupStatePayload {
        gui_ready: state_lock.gui_ready,
        mic_permission: state_lock.mic_permission,
    };

    // Also emit the current state
    app.emit(common::events::health::SETUP_STATE, payload.clone())
        .map_err(|e| HealthError::Emit {
            event: common::events::health::SETUP_STATE.to_string(),
            message: e.to_string(),
        })?;

    Ok(payload)
}

/// Check if both GUI and backend are ready
///
/// Hide `splashscreen` window
///
/// Show `main` window
fn maybe_toggle_windows(
    state_lock: &MutexGuard<'_, SetupState>,
    app: &AppHandle,
) -> common::error::Result<()> {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if state_lock.gui_ready {
        log::info!("Both GUI and backend ready, closing splashscreen and showing main window");

        // Close splashscreen and show main window
        if let Some(splash_window) = app.get_webview_window("splashscreen") {
            splash_window
                .close()
                .map_err(|e| HealthError::WindowOp { op: "close_splashscreen".into(), message: e.to_string() })?;
        }

        if let Some(main_window) = app.get_webview_window("main") {
            main_window
                .show()
                .map_err(|e| HealthError::WindowOp { op: "show_main".into(), message: e.to_string() })?;

            #[cfg(debug_assertions)]
            main_window.open_devtools();

            #[cfg(not(debug_assertions))]
            main_window
                .set_focus()
                .map_err(|e| HealthError::WindowOp { op: "focus_main".into(), message: e.to_string() })?;
        }

        app.emit(common::events::health::APP_READY, ())
            .map_err(|e| HealthError::Emit { event: common::events::health::APP_READY.to_string(), message: e.to_string() })?;

        log::debug!("Emited: {}", common::events::health::APP_READY);
    } else {
        log::debug!(
            "GUI ready: {}.",
            state_lock.gui_ready,
        );
    }

    Ok(())
}
