use shared::instrument::events::{ActivationSourcePayload, PlaybackStatePayload};
use tauri::{AppHandle, Emitter, State};

use crate::{health::HealthSetupState, instrument::engine::{ActivationSource, InstrumentEngine}};


#[tauri::command]
/// Creates instrument engine and starts streaming
pub async fn instrument_playback_start(state: State<'_, InstrumentEngine>) -> Result<(), String> {
    todo!()
}

#[tauri::command]
/// Stops stream and destroys instrument engine
pub async fn instrument_playback_stop(state: State<'_, InstrumentEngine>) -> Result<(), String> {
    todo!()
}

#[tauri::command]
/// Returns true if playback is active
pub async fn instrument_playback_state(state: State<'_, InstrumentEngine>) -> Result<bool, String> {
    let playing = state.inner.playing.lock().await;
    Ok(*playing)
}

#[tauri::command]
/// Returns current instrument activation source (mic or entropy)
pub async fn instrument_activation_source(state: State<'_, InstrumentEngine>) -> Result<ActivationSourcePayload, String> {
    let src = *state.inner.activation_source.lock().await;
    Ok(ActivationSourcePayload { source: src.into() })
}

#[tauri::command]
/// Returns current instrument activation source (mic or entropy)
pub async fn instrument_set_activation_source(source: u8, state: State<'_, InstrumentEngine>, health: State<'_, HealthSetupState>, app: AppHandle) -> Result<(), String> {
    let hs_state = health.lock().await;
    let source: ActivationSource = source.into();
    let mut engine_src = state.inner.activation_source.lock().await;

    match source {
        ActivationSource::Entropy => {
            *engine_src = source;
            app.emit(shared::instrument::events::ACTIVATION_SRC, ActivationSourcePayload{source: source.into()}).map_err(|_| "failed to emit".to_string())?;
        },
        ActivationSource::Mic => {
            if hs_state.mic_permission == Some(true) {
                *engine_src = source;
                app.emit(shared::instrument::events::ACTIVATION_SRC, ActivationSourcePayload{source: source.into()}).map_err(|_| "failed to emit".to_string())?;
            }
            else {
                log::warn!("Won't enable mic activation source without mic permission");
                app.emit(shared::instrument::events::ACTIVATION_SRC, ActivationSourcePayload{source: (*engine_src).into()}).map_err(|_| "failed to emit".to_string())?;
            }
        },
    }


    Ok(())
}

#[tauri::command]
/// Pauses playback, maintaining state
pub async fn instrument_playback_pause(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<(), String> {
    let mut playing = state.inner.playing.lock().await;
    if *playing {
        // Here you would add the logic to pause the playback in your engine
        *playing = false;
        app.emit(shared::instrument::events::PLAYBACK_STATE, PlaybackStatePayload{playing: *playing}).map_err(|_| "failed to emit".to_string())?;
    }

    Ok(())
}

#[tauri::command]
/// Resumes playback from paused state
pub async fn instrument_playback_resume(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<(), String> {
    let mut playing = state.inner.playing.lock().await;
    if !*playing {
        // Here you would add the logic to resume the playback in your engine
        *playing = true;
        app.emit(shared::instrument::events::PLAYBACK_STATE, PlaybackStatePayload{playing: *playing}).map_err(|_| "failed to emit".to_string())?;
    }

    Ok(())
}
