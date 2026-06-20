use std::sync::Arc;

use tauri::State;

use crate::audio_runtime::{stream::PlaybackQuality, DeviceData, PlaybackState};
use crate::dsp::DspState;

#[tauri::command]
pub async fn start_playback(
    engine: State<'_, Arc<super::AudioEngine>>,
    dsp_state: State<'_, DspState>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    log::debug!(
        "Starting playback with DSP state: synth={:?}, analyze={:?}",
        dsp_state.synth.read().is_some(),
        dsp_state.analyze.read().is_some()
    );
    engine.start(
        dsp_state.analyze.read().clone(),
        dsp_state.synth.read().clone(),
    )?;
    log::info!("Playback started");
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn stop_playback(
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    engine.stop()?;
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn pause_playback(
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    engine.pause()?;
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn resume_playback(
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    engine.resume()?;
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn list_audio_devices(
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<Vec<DeviceData>, super::AudioRuntimeError> {
    Ok(engine.list_devices())
}

#[tauri::command]
pub async fn select_input_device(
    host_id: String,
    device_id: String,
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    engine.select_input_device(host_id, device_id)?;
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn select_output_device(
    host_id: String,
    device_id: String,
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    engine.select_output_device(host_id, device_id)?;
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn set_quality(
    quality: PlaybackQuality,
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    engine.on_quality_change(quality)?;
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn get_playback_state(
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackState, super::AudioRuntimeError> {
    Ok(engine.playback_state())
}

#[tauri::command]
pub async fn get_current_quality(
    engine: State<'_, Arc<super::AudioEngine>>,
) -> Result<PlaybackQuality, super::AudioRuntimeError> {
    Ok(engine.current_quality())
}
