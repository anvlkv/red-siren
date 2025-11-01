use common::error::{Result, TunerError};
use common::tuner::{Config, Layout as TunerLayout, SpectrumData};
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
    app: AppHandle,
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
    app: AppHandle,
    key: NodeKey,
    min_frequency: f32,
    max_frequency: f32,
    min_magnitude: f32,
    max_magnitude: f32,
) -> Result<()> {
    // Update tuner config
    _ = state.update_sensor_valuess(
        key,
        min_frequency,
        max_frequency,
        min_magnitude,
        max_magnitude,
    )?;

    Ok(())
}

#[tauri::command]
pub fn tuner_reset_config(
    state: State<'_, TunerState>,
    instrument: State<'_, InstrumentEngine>,
    app: AppHandle,
) -> Result<()> {
    let inst_layout = instrument.layout();
    let sample_rate = instrument.sample_rate() as f32;
    // Generate default tuner config from layout
    let tuner_layout: TunerLayout = inst_layout.into();
    let new_config = Config::new(tuner_layout, sample_rate, inst_layout.registry());

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
