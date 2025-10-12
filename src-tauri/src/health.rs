use serde_json::Value;
use tauri::{App, AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use parking_lot::{Mutex, MutexGuard}; // switched from tokio::sync::Mutex to parking_lot for non-async, faster locking
use common::error::{HealthError, AppError};

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
    use cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        *,
    };
    use std::sync::Arc;

    async fn run<S>(device: &Device, config: &StreamConfig) -> std::result::Result<(), HealthError>
        where
            S: SizedSample + FromSample<f32>,
        {
            // Shared place for the error callback to deposit any stream error.
            let err_flag = Arc::new(std::sync::Mutex::new(None::<String>));

            // Clone items we need to move into the blocking closure.
            let device = device.clone();
            let config = config.clone();
            let err_flag_for_thread = err_flag.clone();

            // Run the stream lifecycle on a blocking thread so non-Send callbacks/stream
            // do not live across .await points on the async runtime.
            let join_handle = tokio::task::spawn_blocking(move || -> std::result::Result<(), HealthError> {
                // Data callback — intentionally empty.
                let data_callback = move |_: &[S], _: &InputCallbackInfo| {};

                // Error callback writes into the shared flag.
                let err_flag_for_cb = err_flag_for_thread.clone();
                let err_callback = move |err: StreamError| {
                    if let Ok(mut guard) = err_flag_for_cb.lock() {
                        if guard.is_none() {
                            *guard = Some(format!("Stream error: {err}"));
                        }
                    }
                };

                // Build stream (map CPAL errors).
                let stream = device
                    .build_input_stream::<S, _, _>(
                        &config,
                        data_callback,
                        err_callback,
                        Some(std::time::Duration::from_secs(1)),
                    )
                    .map_err(|e| HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) })?;

                stream
                    .play()
                    .map_err(|e| HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) })?;

                // Sleep on the blocking thread so we don't block the async reactor.
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Drop stream before inspecting error flag.
                drop(stream);

                // Check if the callback reported an error.
                if let Some(err_str) = err_flag_for_thread.lock().unwrap().take() {
                    return Err(HealthError::MicPermissionCheckFailed { detail: Some(err_str) });
                }

                Ok(())
            });

            // Await the blocking task's result on the async runtime.
            match join_handle.await {
                Ok(inner) => inner,
                Err(e) => Err(HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) }),
            }
        }

    async fn check() -> std::result::Result<(), HealthError> {
        let host = default_host();

        // Handle lack of default input device explicitly.
        let device = match host.default_input_device() {
            Some(d) => d,
            None => return Err(HealthError::MicPermissionCheckFailed { detail: Some("no_input_device".into()) }),
        };

        let cfg = device
            .default_input_config()
            .map_err(|e| HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) })?;

        // Convert to concrete StreamConfig to avoid borrowing a temporary.
        let stream_cfg: StreamConfig = cfg.clone().into();

        match cfg.sample_format() {
            SampleFormat::F32 => run::<f32>(&device, &stream_cfg).await?,
            SampleFormat::I16 => run::<i16>(&device, &stream_cfg).await?,
            SampleFormat::U16 => run::<u16>(&device, &stream_cfg).await?,
            f => return Err(HealthError::MicPermissionCheckFailed { detail: Some(format!("unsupported_sample_format:{f:?}")) }),
        }

        Ok(())
    }

    // If prompting, run full check and propagate any HealthError (mapped automatically into AppError).
    let check_result = if prompt {
        // Explicitly wrap HealthError into AppError (even though the From impl exists),
        // making the intent clear at this integration boundary.
        check().await.map_err(|e| AppError::from(HealthError::MicPermissionCheckFailed { detail: Some(format!("{e}")) }))?;
        true
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
