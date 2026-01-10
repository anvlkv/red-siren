use common::error::Result;
use common::tuner::{Config, Layout as TunerLayout, ReflectTunerConstraints, SpectrumData};
use common::NodeKey;
use tauri::{AppHandle, State};

use crate::instrument::InstrumentEngine;

use super::TunerState;

#[tauri::command]
pub fn tuner_config(state: State<'_, TunerState>) -> Result<Config> {
    Ok(state.tuner_config())
}

#[tauri::command]
pub fn tuner_layout(
    state: State<'_, TunerState>,
    instrument: State<'_, InstrumentEngine>,
) -> Result<TunerLayout> {
    if let Some(l) = state.tuner_layout() {
        return Ok(l);
    }

    let instrument_layout = instrument.layout();
    let tuner_layout: TunerLayout = instrument_layout.into();
    let registry = instrument_layout.registry();
    _ = state.update_layout(tuner_layout, registry)?;

    Ok(tuner_layout)
}

#[tauri::command]
pub fn tuner_spectrum_data(app: AppHandle) -> Result<SpectrumData> {
    Ok(TunerState::spectrum_data(&app))
}

#[tauri::command]
pub fn tuner_update_sensor(
    state: State<'_, TunerState>,
    keys: Vec<NodeKey>,
    min_frequency_increment: f32,
    max_frequency_increment: f32,
    min_magnitude_increment: f32,
    max_magnitude_increment: f32,
) -> Result<Config> {
    let mut cfg = state.tuner_config();
    let eps = f32::EPSILON.sqrt();
    let (min_freq, max_freq) = cfg.frequency_range_limits();
    // Update tuner config
    for key in keys {
        if let Some(old_value) = cfg.sensor_data.iter().find(|s| s.key == key).copied() {
            let min_freq = (old_value.min_frequency + min_frequency_increment).max(min_freq);
            let max_freq = (old_value.max_frequency + max_frequency_increment).min(max_freq);
            let min_mag = (old_value.min_magnitude + min_magnitude_increment).clamp(0.0, 1.0);
            let max_mag = (old_value.max_magnitude + max_magnitude_increment).clamp(0.0, 1.0);
            cfg = state.update_sensor_valuess(
                key,
                min_freq.min(max_freq - eps),
                max_freq.max(min_freq + eps),
                min_mag.min(max_mag - eps),
                max_mag.max(min_mag + eps),
            )?;
        }
    }

    Ok(cfg)
}

#[tauri::command]
pub fn tuner_reset_config(
    state: State<'_, TunerState>,
    instrument: State<'_, InstrumentEngine>,
) -> Result<()> {
    let inst_layout = instrument.layout();
    let sample_rate = instrument.sample_rate() as f32;
    // Generate default tuner config from layout
    let tuner_layout: TunerLayout = inst_layout.into();
    let new_config = Config::new(
        tuner_layout,
        sample_rate,
        audio_system::FFT_WINDOW_SIZE,
        inst_layout.registry(),
    );

    // Update tuner config
    state.reset(&new_config, &tuner_layout)?;

    Ok(())
}

#[tauri::command]
pub fn tuner_start_stream(state: State<'_, TunerState>) -> Result<()> {
    state.start_tuner_stream()
}

#[tauri::command]
pub fn tuner_stop_stream(state: State<'_, TunerState>) -> Result<()> {
    state.stop_tuner_stream()
}

#[tauri::command]
pub fn tuner_toggle_probe(state: State<'_, TunerState>) -> Result<bool> {
    state.toggle_probe()
}

#[tauri::command]
pub fn tuner_constraints_updated(state: State<'_, TunerState>) -> Result<ReflectTunerConstraints> {
    Ok(state.tuner_config().constraints())
}

#[tauri::command]
pub fn tuner_update_range(
    min_frequency: Option<f32>,
    max_frequency: Option<f32>,
    state: State<'_, TunerState>,
) -> Result<()> {
    state.update_range(min_frequency, max_frequency)
}

#[tauri::command]
pub fn tuner_update_threshold(ny_threshold: f32, state: State<'_, TunerState>) -> Result<()> {
    state.update_ny_threshold(ny_threshold)
}

#[tauri::command]
pub fn tuner_update_wet_ratio(wet_ratio: f32, state: State<'_, TunerState>) -> Result<()> {
    state.update_wet_ratio(wet_ratio)
}

#[tauri::command]
pub fn tuner_snapshot_input_snoop(state: State<'_, TunerState>) -> Result<Vec<f32>> {
    state.snapshot_input_snoop()
}
