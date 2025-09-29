use serde_json::Value;
use tauri::{App, AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Default, Debug, Clone, Copy)]
/// State to track setup completion
pub struct SetupState {
    pub gui_ready: bool,
    pub mic_permission: Option<bool>,
}

const HEALTH_STORE_NAME:&str = "health.json";
const MIC_PEMISSION_KEY:&str = "mic_permission";

pub fn setup(app: &mut App) -> tauri_plugin_store::Result<()> {
    let store = app.store(HEALTH_STORE_NAME)?;

    let mic_permission = store.get(MIC_PEMISSION_KEY).and_then(|v: Value| v.as_bool());

    app.manage(Mutex::new(SetupState {
        mic_permission,
        ..Default::default()
    }));

    Ok(())
}

#[tauri::command]
pub async fn health_grant_mic_premission(
    app: AppHandle,
    state: State<'_, Mutex<SetupState>>,
    prompt: bool,
) -> Result<bool, String> {
    use cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        *,
    };
    use std::sync::Arc;

    async fn run<S>(device: &Device, config: &StreamConfig) -> Result<(), String>
        where
            S: SizedSample + FromSample<f32>,
        {
            // Shared place for the error callback to deposit any stream error.
                let err_flag = Arc::new(std::sync::Mutex::new(None::<String>));

                // Clone items we need to move into the blocking closure.
                // (Device and StreamConfig must be Clone — if they are not, adapt to take ownership.)
                let device = device.clone();
                let config = config.clone();
                let err_flag_for_thread = err_flag.clone();

                // Run the stream lifecycle on a blocking thread so non-Send callbacks/stream
                // do not live across .await points on the async runtime.
                let join_handle = tokio::task::spawn_blocking(move || -> Result<(), String> {
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

                    // Build stream (map CPAL errors to String).
                    let stream = device
                        .build_input_stream::<S, _, _>(
                            &config,
                            data_callback,
                            err_callback,
                            Some(std::time::Duration::from_secs(1)),
                        )
                        .map_err(|e| format!("Unable to create input stream: {e}"))?;

                    stream
                        .play()
                        .map_err(|e| format!("Unable to start input stream: {e}"))?;

                    // Sleep on the blocking thread so we don't block the async reactor.
                    std::thread::sleep(std::time::Duration::from_millis(200));

                    stream
                        .pause()
                        .map_err(|e| format!("Unable to stop input stream: {e}"))?;

                    // Drop stream here (explicitly) so callbacks are dropped before we inspect the error flag.
                    drop(stream);

                    // Check if the callback reported an error.
                    if let Some(err_str) = err_flag_for_thread.lock().unwrap().take() {
                        Err(err_str)
                    } else {
                        Ok(())
                    }
                });

                // Await the blocking task's result on the async runtime.
                // map the JoinError to a string, and propagate the inner Result.
                match join_handle.await {
                    Ok(inner) => inner,
                    Err(e) => Err(format!("Blocking task join error: {e}")),
                }
        }

    async fn check() -> Result<(), String> {
        let host = default_host();

        // Handle lack of default input device explicitly to keep error types consistent.
        let device = match host.default_input_device() {
            Some(d) => d,
            None => return Err("No input device found...".to_string()),
        };

        let cfg = device
            .default_input_config()
            .map_err(|e| format!("Unable to configure input device: {e}"))?;

        // Convert to concrete StreamConfig to avoid borrowing a temporary in match arms.
        let stream_cfg: StreamConfig = cfg.clone().into();

        match cfg.sample_format() {
            SampleFormat::F32 => run::<f32>(&device, &stream_cfg).await?,
            SampleFormat::I16 => run::<i16>(&device, &stream_cfg).await?,
            SampleFormat::U16 => run::<u16>(&device, &stream_cfg).await?,
            f => return Err(format!("Unsupported sample format: [{f:?}]")),
        }

        Ok(())
    }

    let check_result = if prompt {
        check().await.is_ok()
    } else {
        false
    };

    let mut health_state = state.lock().await;

    health_state.mic_permission = Some(check_result);

    let store = app.store(HEALTH_STORE_NAME).unwrap();

    store.set(MIC_PEMISSION_KEY, check_result);

    Ok(check_result)
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

/// Check if both GUI and backend are ready
///
/// Hide `splashscreen` window
///
/// Show `main` window
fn maybe_toggle_windows(
    state_lock: &MutexGuard<'_, SetupState>,
    app: &AppHandle,
) -> Result<(), String> {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if state_lock.gui_ready {
        log::info!("Both GUI and backend ready, closing splashscreen and showing main window");

        // Close splashscreen and show main window
        if let Some(splash_window) = app.get_webview_window("splashscreen") {
            splash_window
                .close()
                .map_err(|e| format!("Failed to close splashscreen: {}", e))?;
        }

        if let Some(main_window) = app.get_webview_window("main") {
            main_window
                .show()
                .map_err(|e| format!("Failed to show main window: {}", e))?;

            #[cfg(not(debug_assertions))]
            main_window
                .set_focus()
                .map_err(|e| format!("Failed to focus main window: {}", e))?;
        }

        app.emit(shared::events::health::APP_READY, ())
            .map_err(|e| e.to_string())?;

        log::debug!("Emited: {}", shared::events::health::APP_READY);
    } else {
        log::debug!(
            "GUI ready: {}.",
            state_lock.gui_ready,
        );
    }

    Ok(())
}
