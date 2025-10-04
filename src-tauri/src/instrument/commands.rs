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

    match state.inner.start_playback()? {
        true => {
            log::info!("Starting playback");
            app.emit(
                common::instrument::events::PLAYBACK_STATE,
                PlaybackStatePayload { playing: true },
            )
            .map_err(|e| InstrumentError::ResumeFailed {
                detail: Some(e.to_string()),
            })?;
            log::info!("Emitted playback state: playing");
        }
        false => {
            log::warn!("Playback already active; no action taken");
        }
    }

    Ok(())
}

#[tauri::command]
/// Stops stream and destroys instrument engine
pub fn instrument_playback_stop(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_stop called");

    match state.inner.stop_playback()? {
        true => {
            log::info!("Stopping playback");
            app.emit(
                common::instrument::events::PLAYBACK_STATE,
                PlaybackStatePayload { playing: false },
            )
            .map_err(|e| InstrumentError::ResumeFailed {
                detail: Some(e.to_string()),
            })?;
            log::info!("Emitted playback state: stopped");
        }
        false => {
            log::warn!("Playback already stopped; no action taken");
        }
    }

    Ok(())
}

#[tauri::command]
/// Returns true if playback is active
pub fn instrument_playback_state(
    state: State<'_, InstrumentEngine>,
) -> Result<PlaybackStatePayload> {
    log::debug!("instrument_playback_state called");
    let playing = state.inner.playing();
    log::debug!("Returning playback state: {}", playing);
    Ok(PlaybackStatePayload { playing })
}

#[tauri::command]
/// Returns current instrument activation source (mic or entropy)
pub fn instrument_activation_source(
    state: State<'_, InstrumentEngine>,
) -> Result<ActivationSourcePayload> {
    log::debug!("instrument_activation_source called");
    let source = state.inner.activation_source();
    log::debug!("Current activation source (enum): {:?}", source);
    Ok(ActivationSourcePayload { source })
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
    let requested: ActivationSource = source.into();
    let current: ActivationSource = state.inner.activation_source().into();

    // If no change, just log and return
    if current == requested {
        log::debug!(
            "Activation source unchanged (still {:?}, code={}) - no action taken",
            current,
            src_u8
        );
        return Ok(());
    }

    // Permission check if Mic requested
    if matches!(requested, ActivationSource::Mic) && hs_state.mic_permission != Some(true) {
        log::warn!("Won't enable mic activation source without mic permission");
        return Err(InstrumentError::MicPermissionMissing.into());
    }

    // Apply change via inner method
    match state.inner.set_activation_source(requested)? {
        true => {
            log::info!(
                "Setting activation source to {:?} (code={})",
                requested,
                src_u8
            );
            app.emit(
                common::instrument::events::ACTIVATION_SRC,
                ActivationSourcePayload {
                    source: requested.into(),
                },
            )
            .map_err(|e| InstrumentError::Emit {
                event: common::instrument::events::ACTIVATION_SRC.to_string(),
                message: e.to_string(),
            })?;
            log::info!("Emitted activation source event: code={}", src_u8);
        }
        false => {
            log::warn!(
                "Inner reported activation source not changed for {:?} (code={})",
                requested,
                src_u8
            );
        }
    }

    Ok(())
}

#[tauri::command]
/// Pauses playback, maintaining state
pub fn instrument_playback_pause(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_pause called");

    match state.inner.pause_playback()? {
        true => {
            log::info!("Pausing playback");
            app.emit(
                common::instrument::events::PLAYBACK_STATE,
                PlaybackStatePayload { playing: false },
            )
            .map_err(|e| InstrumentError::PauseFailed {
                detail: Some(e.to_string()),
            })?;
            log::info!("Emitted playback state: playing=false");
        }
        false => {
            log::warn!("Playback already paused; no action taken");
        }
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

    match state.inner.resume_playback()? {
        true => {
            log::info!("Resuming playback");
            app.emit(
                common::instrument::events::PLAYBACK_STATE,
                PlaybackStatePayload { playing: true },
            )
            .map_err(|e| InstrumentError::ResumeFailed {
                detail: Some(e.to_string()),
            })?;
            log::info!("Emitted playback state: playing=true");
        }
        false => {
            log::warn!("Playback already running; no action taken");
        }
    }

    Ok(())
}

#[tauri::command]
/// Returns current instrument layout (invoke/event: instrument_layout)
pub fn instrument_layout(state: State<'_, InstrumentEngine>) -> Result<Layout> {
    log::debug!("instrument_layout called");
    Ok(state.inner.layout())
}

#[tauri::command]
/// payload: `SafeArea`
pub fn ui_safe_area_insets_apply(
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
    state: State<'_, InstrumentEngine>,
    window_state: State<'_, crate::setup::WindowState>,
    app: AppHandle,
) -> Result<()> {
    log::debug!(
        "ui_safe_area_insets_apply called (UI override): top={}, right={}, bottom={}, left={}",
        top,
        right,
        bottom,
        left
    );

    {
        // Persist UI safe area contribution in window state (non-additive override)
        let mut win = window_state.lock();
        win.ui_safe_area = shared::safe_area::SafeArea {
            top,
            right,
            bottom,
            left,
        };
    }

    // Get current layout before changes
    let old_layout = state.inner.layout();

    state
        .inner
        .set_safe_area(top, right, bottom, left)
        .map_err(|e| InstrumentError::Emit {
            event: "set_safe_area".to_string(),
            message: e.to_string(),
        })?;

    let new_layout = state.inner.layout();

    // Only emit LAYOUT event if layout actually changed
    if old_layout != new_layout {
        log::info!(
            "Applied UI safe area override [top: {}, right: {}, bottom: {}, left: {}] - layout changed",
            top,
            right,
            bottom,
            left
        );

        app.emit(common::instrument::events::LAYOUT, new_layout)
            .map_err(|e| InstrumentError::Emit {
                event: common::instrument::events::LAYOUT.to_string(),
                message: e.to_string(),
            })?;
    } else {
        log::debug!(
            "UI safe area values unchanged [top: {}, right: {}, bottom: {}, left: {}] - skipping layout emission",
            top,
            right,
            bottom,
            left
        );
    }

    Ok(())
}
