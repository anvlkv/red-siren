use audio_system::rt::{check_mic_permission, supports_mic};
use crate::persistence::persistence::{load_bool, save_bool};
use tauri::{App, AppHandle, Emitter, Manager, State};

use parking_lot::{Mutex, MutexGuard}; // switched from tokio::sync::Mutex to parking_lot for non-async, faster locking
use common::error::{HealthError, AppError};

#[derive(Default, Debug, Clone, Copy)]
/// State to track setup completion
pub struct SetupState {
    pub gui_ready: bool,
    pub initial_mic_permission: Option<bool>,
}

const HEALTH_STORE_NAME:&str = "health.json";
const INITIAL_MIC_PERMISSION_KEY:&str = "initial_mic_permission";
const LEGACY_MIC_PERMISSION_KEY:&str = "mic_permission";

pub type HealthSetupState = Mutex<SetupState>;

pub fn setup(app: &mut App) -> common::error::Result<()> {
    let (initial_mic_permission, migrated_legacy_key) =
        match load_bool(app.handle(), HEALTH_STORE_NAME, INITIAL_MIC_PERMISSION_KEY)? {
            Some(value) => (Some(value), false),
            None => {
                let legacy = load_bool(app.handle(), HEALTH_STORE_NAME, LEGACY_MIC_PERMISSION_KEY)?;
                (legacy, legacy.is_some())
            }
        };

    let initial_state = SetupState {
        initial_mic_permission,
        ..Default::default()
    };

    if migrated_legacy_key {
        if let Some(value) = initial_state.initial_mic_permission {
            save_bool(&app.handle(), HEALTH_STORE_NAME, INITIAL_MIC_PERMISSION_KEY, value)?;
        }
    }

    // Emit initial setup state
    app.emit(
        common::events::health::SETUP_STATE,
        common::commands::health::SetupStatePayload {
            gui_ready: initial_state.gui_ready,
            initial_mic_permission: initial_state.initial_mic_permission,
            devtools: cfg!(feature="devtools")
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
        // Explicitly wrap HealthError into AppError (even though the From impl exists),
        // making the intent clear at this integration boundary.
        if supports_mic() {
            check_mic_permission().await.map_err(|e| AppError::from(HealthError::MicPermissionCheckFailed { detail: Some(format!("{e}")) }))?;
            true
        } else {
            false
        }
    } else {
        false
    };

    let mut health_state = state.lock();

    health_state.initial_mic_permission = Some(check_result);

    // Emit the updated setup state
    app.emit(
        common::events::health::SETUP_STATE,
        common::commands::health::SetupStatePayload {
            gui_ready: health_state.gui_ready,
            initial_mic_permission: health_state.initial_mic_permission,
            devtools: cfg!(feature="devtools")
        },
    )
    .map_err(|e| HealthError::Emit {
        event: common::events::health::SETUP_STATE.to_string(),
        message: e.to_string(),
    })?;

    save_bool(&app, HEALTH_STORE_NAME, INITIAL_MIC_PERMISSION_KEY, check_result)?;



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
            initial_mic_permission: state_lock.initial_mic_permission,
            devtools: cfg!(feature="devtools")
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
    // Snapshot payload while holding the lock, then drop lock before emitting
    let payload = {
        let state_lock = state.lock();
        common::commands::health::SetupStatePayload {
            gui_ready: state_lock.gui_ready,
            initial_mic_permission: state_lock.initial_mic_permission,
            devtools: cfg!(feature="devtools")
        }
    };

    // Emit the current state
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

            #[cfg(any(debug_assertions, feature = "devtools"))]
            main_window.open_devtools();

            #[cfg(not(debug_assertions))]
            main_window
                .set_focus()
                .map_err(|e| HealthError::WindowOp { op: "focus_main".into(), message: e.to_string() })?;
        }
    } else {
        log::debug!(
            "GUI ready: {}.",
            state_lock.gui_ready,
        );
    }

    Ok(())
}
