use shared::commands::setup::SafeAreaInstestUiIncrementPayload;
use shared::error::{InstrumentError, Result};
use shared::instrument::{
    events::{ActivationSourcePayload, PlaybackStatePayload},
    Layout,
};
use tauri::{AppHandle, Emitter, State};

use crate::{
    health::HealthSetupState,
    instrument::engine::{ActivationSource, InstrumentEngine},
};

#[tauri::command]
/// Creates instrument engine and starts streaming
pub fn instrument_playback_start(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_start called");
    let mut playing = state.inner.playing.lock();
    log::debug!("Current playing state: {}", *playing);

    if !*playing {
        *playing = true;
        log::info!("Starting playback");
        app.emit(
            shared::instrument::events::PLAYBACK_STATE,
            PlaybackStatePayload { playing: *playing },
        )
        .map_err(|e| InstrumentError::ResumeFailed {
            detail: Some(e.to_string()),
        })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already active; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Stops stream and destroys instrument engine
pub fn instrument_playback_stop(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_stop called");
    let mut playing = state.inner.playing.lock();
    log::debug!("Current playing state: {}", *playing);

    if !*playing {
        *playing = false;
        log::info!("Stopping playback");
        app.emit(
            shared::instrument::events::PLAYBACK_STATE,
            PlaybackStatePayload { playing: *playing },
        )
        .map_err(|e| InstrumentError::ResumeFailed {
            detail: Some(e.to_string()),
        })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already stopped; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Returns true if playback is active
pub fn instrument_playback_state(
    state: State<'_, InstrumentEngine>,
) -> Result<PlaybackStatePayload> {
    log::debug!("instrument_playback_state called");
    let playing = state.inner.playing.lock();
    log::debug!("Returning playback state: {}", *playing);
    Ok(PlaybackStatePayload { playing: *playing })
}

#[tauri::command]
/// Returns current instrument activation source (mic or entropy)
pub fn instrument_activation_source(
    state: State<'_, InstrumentEngine>,
) -> Result<ActivationSourcePayload> {
    log::debug!("instrument_activation_source called");
    let src = *state.inner.activation_source.lock();
    log::debug!("Current activation source (enum): {:?}", src);
    Ok(ActivationSourcePayload { source: src.into() })
}

#[tauri::command]
/// Returns current instrument activation source (mic or entropy)
pub fn instrument_set_activation_source(
    source: u8,
    state: State<'_, InstrumentEngine>,
    health: State<'_, HealthSetupState>,
    app: AppHandle,
) -> Result<()> {
    log::debug!(
        "instrument_set_activation_source called with source={}",
        source
    );
    let hs_state = health.lock();
    let src_u8 = source;
    let source: ActivationSource = source.into();
    let mut engine_src = state.inner.activation_source.lock();

    match source {
        ActivationSource::Entropy => {
            *engine_src = source;
            log::info!("Setting activation source to Entropy (code={})", src_u8);
            app.emit(
                shared::instrument::events::ACTIVATION_SRC,
                ActivationSourcePayload {
                    source: source.into(),
                },
            )
            .map_err(|e| InstrumentError::Emit {
                event: shared::instrument::events::ACTIVATION_SRC.to_string(),
                message: e.to_string(),
            })?;
            log::info!("Emitted activation source event: code={}", src_u8);
        }
        ActivationSource::Mic => {
            if hs_state.mic_permission == Some(true) {
                *engine_src = source;
                log::info!("Setting activation source to Mic (code={})", src_u8);
                app.emit(
                    shared::instrument::events::ACTIVATION_SRC,
                    ActivationSourcePayload {
                        source: source.into(),
                    },
                )
                .map_err(|e| InstrumentError::Emit {
                    event: shared::instrument::events::ACTIVATION_SRC.to_string(),
                    message: e.to_string(),
                })?;
                log::info!("Emitted activation source event: code={}", src_u8);
            } else {
                log::warn!("Won't enable mic activation source without mic permission");
                return Err(InstrumentError::MicPermissionMissing.into());
            }
        }
    }

    Ok(())
}

#[tauri::command]
/// Pauses playback, maintaining state
pub fn instrument_playback_pause(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_pause called");
    let mut playing = state.inner.playing.lock();
    log::debug!("Current playing state: {}", *playing);

    if *playing {
        *playing = false;
        log::info!("Pausing playback");
        app.emit(
            shared::instrument::events::PLAYBACK_STATE,
            PlaybackStatePayload { playing: *playing },
        )
        .map_err(|e| InstrumentError::PauseFailed {
            detail: Some(e.to_string()),
        })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already paused; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Resumes playback from paused state
pub fn instrument_playback_resume(
    state: State<'_, InstrumentEngine>,
    app: AppHandle,
) -> Result<()> {
    log::debug!("instrument_playback_resume called");
    let mut playing = state.inner.playing.lock();
    log::debug!("Current playing state: {}", *playing);

    if !*playing {
        *playing = true;
        log::info!("Resuming playback");
        app.emit(
            shared::instrument::events::PLAYBACK_STATE,
            PlaybackStatePayload { playing: *playing },
        )
        .map_err(|e| InstrumentError::ResumeFailed {
            detail: Some(e.to_string()),
        })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already running; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Returns current instrument layout (invoke/event: instrument_layout)
pub fn instrument_layout(state: State<'_, InstrumentEngine>) -> Result<Layout> {
    log::debug!("instrument_layout called");
    Ok(*state.inner.layout.lock())
}

#[tauri::command]
pub fn ui_safe_area_insets_apply(
    payload: SafeAreaInstestUiIncrementPayload,
    state: State<'_, InstrumentEngine>,
    window_state: State<'_, crate::setup::WindowState>,
    app: AppHandle,
) -> Result<()> {
    log::debug!(
        "ui_safe_area_insets_apply called (UI override): top={}, right={}, bottom={}, left={}",
        payload.top,
        payload.right,
        payload.bottom,
        payload.left
    );

    {
        // Persist UI safe area contribution in window state (non-additive override)
        let mut win = window_state.lock();
        win.ui_safe_area = payload;
    }

    state
        .set_safe_area(payload.top, payload.right, payload.bottom, payload.left)
        .map_err(|e| InstrumentError::Emit {
            event: "set_safe_area".to_string(),
            message: e.to_string(),
        })?;

    let layout = *state.inner.layout.lock();
    log::info!(
        "Applied UI safe area override [top: {}, right: {}, bottom: {}, left: {}]",
        payload.top,
        payload.right,
        payload.bottom,
        payload.left
    );

    app.emit(shared::instrument::events::LAYOUT, layout)
        .map_err(|e| InstrumentError::Emit {
            event: shared::instrument::events::LAYOUT.to_string(),
            message: e.to_string(),
        })?;

    Ok(())
}
