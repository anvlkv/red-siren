use shared::{events::setup::SafeAreaInstestUiIncrementPayload, instrument::{events::{ActivationSourcePayload, PlaybackStatePayload}, Layout}};
use shared::error::{Result, InstrumentError};
use tauri::{AppHandle, Emitter, State};

use crate::{health::HealthSetupState, instrument::engine::{ActivationSource, InstrumentEngine}};


#[tauri::command]
/// Creates instrument engine and starts streaming
pub async fn instrument_playback_start(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_start called");
    let mut playing = state.inner.playing.lock().await;
    log::debug!("Current playing state: {}", *playing);

    if !*playing {
        // Here you would add the logic to resume the playback in your engine
        *playing = true;
        log::info!("Starting playback");
        app.emit(shared::instrument::events::PLAYBACK_STATE, PlaybackStatePayload{playing: *playing})
            .map_err(|e| InstrumentError::ResumeFailed { detail: Some(e.to_string()) })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already active; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Stops stream and destroys instrument engine
pub async fn instrument_playback_stop(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_stop called");
    let mut playing = state.inner.playing.lock().await;
    log::debug!("Current playing state: {}", *playing);

    if !*playing {
        // Here you would add the logic to resume the playback in your engine
        *playing = false;
        log::info!("Stopping playback");
        app.emit(shared::instrument::events::PLAYBACK_STATE, PlaybackStatePayload{playing: *playing})
            .map_err(|e| InstrumentError::ResumeFailed { detail: Some(e.to_string()) })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already stopped; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Returns true if playback is active
pub async fn instrument_playback_state(state: State<'_, InstrumentEngine>) -> Result<PlaybackStatePayload> {
    log::debug!("instrument_playback_state called");
    let playing = state.inner.playing.lock().await;
    log::debug!("Returning playback state: {}", *playing);
    Ok(PlaybackStatePayload { playing: *playing })
}

#[tauri::command]
/// Returns current instrument activation source (mic or entropy)
pub async fn instrument_activation_source(state: State<'_, InstrumentEngine>) -> Result<ActivationSourcePayload> {
    log::debug!("instrument_activation_source called");
    let src = *state.inner.activation_source.lock().await;
    log::debug!("Current activation source (enum): {:?}", src);
    Ok(ActivationSourcePayload { source: src.into() })
}

#[tauri::command]
/// Returns current instrument activation source (mic or entropy)
pub async fn instrument_set_activation_source(source: u8, state: State<'_, InstrumentEngine>, health: State<'_, HealthSetupState>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_set_activation_source called with source={}", source);
    let hs_state = health.lock().await;
    let src_u8 = source;
    let source: ActivationSource = source.into();
    let mut engine_src = state.inner.activation_source.lock().await;

    match source {
        ActivationSource::Entropy => {
            *engine_src = source;
            log::info!("Setting activation source to Entropy (code={})", src_u8);
            app.emit(shared::instrument::events::ACTIVATION_SRC, ActivationSourcePayload{source: source.into()})
                .map_err(|e| InstrumentError::Emit { event: shared::instrument::events::ACTIVATION_SRC.to_string(), message: e.to_string() })?;
            log::info!("Emitted activation source event: code={}", src_u8);
        },
        ActivationSource::Mic => {
            if hs_state.mic_permission == Some(true) {
                *engine_src = source;
                log::info!("Setting activation source to Mic (code={})", src_u8);
                app.emit(shared::instrument::events::ACTIVATION_SRC, ActivationSourcePayload{source: source.into()})
                    .map_err(|e| InstrumentError::Emit { event: shared::instrument::events::ACTIVATION_SRC.to_string(), message: e.to_string() })?;
                log::info!("Emitted activation source event: code={}", src_u8);
            }
            else {
                log::warn!("Won't enable mic activation source without mic permission");
                return Err(InstrumentError::MicPermissionMissing.into());
            }
        },
    }

    Ok(())
}

#[tauri::command]
/// Pauses playback, maintaining state
pub async fn instrument_playback_pause(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_pause called");
    let mut playing = state.inner.playing.lock().await;
    log::debug!("Current playing state: {}", *playing);

    if *playing {
        // Here you would add the logic to pause the playback in your engine
        *playing = false;
        log::info!("Pausing playback");
        app.emit(shared::instrument::events::PLAYBACK_STATE, PlaybackStatePayload{playing: *playing})
            .map_err(|e| InstrumentError::PauseFailed { detail: Some(e.to_string()) })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already paused; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Resumes playback from paused state
pub async fn instrument_playback_resume(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_resume called");
    let mut playing = state.inner.playing.lock().await;
    log::debug!("Current playing state: {}", *playing);

    if !*playing {
        // Here you would add the logic to resume the playback in your engine
        *playing = true;
        log::info!("Resuming playback");
        app.emit(shared::instrument::events::PLAYBACK_STATE, PlaybackStatePayload{playing: *playing})
            .map_err(|e| InstrumentError::ResumeFailed { detail: Some(e.to_string()) })?;
        log::info!("Emitted playback state: playing={}", *playing);
    } else {
        log::warn!("Playback already running; no action taken");
    }

    Ok(())
}

#[tauri::command]
/// Returns current instrument layout (invoke/event: instrument_layout)
pub async fn instrument_layout(state: State<'_, InstrumentEngine>) -> Result<Layout> {
    log::debug!("instrument_layout called");
    Ok(*state.inner.layout.lock().await)
}

#[tauri::command]
pub async fn ui_safe_area_insets_increment(top: f32, left: f32, right: f32, bottom: f32, state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
    // state.set_safe_area(SafeAreaInstestUiIncrementPayload{ top, right, bottom, left }).await?;
    // let layout = state.inner.layout.lock().await;

    // let mut config = state.inner.config.lock().await;
    // *config = shared::instrument::Config::try_from(*layout)?;

    // log::info!("Updated layout with new safe area [top: {top}, right: {right}, bottom: {bottom}, left: {left}]: {:#?}", *layout);

    // app.emit(shared::instrument::events::LAYOUT, *layout)?;


    Ok(())
}
