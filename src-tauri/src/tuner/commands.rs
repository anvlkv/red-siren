use common::error::{Result, TunerError};
use common::tuner::{Config, Layout as TunerLayout, SpectrumData};
use common::NodeKey;
use tauri::{AppHandle, Emitter, State};

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
    if let Some(updated_config) = state.update_layout(tuner_layout, registry)? {
        // Emit config update event
        app.emit(common::events::tuner::CONFIG, updated_config.clone())
            .map_err(|e| TunerError::Emit {
                event: common::events::tuner::CONFIG.to_string(),
                message: e.to_string(),
            })?;
    }

    Ok(tuner_layout)
}

#[tauri::command]
pub fn tuner_spectrum_data(state: State<'_, TunerState>) -> Result<SpectrumData> {
    Ok(state.spectrum_data())
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
    let updated_config = state.update_sensor_valuess(
        key,
        min_frequency,
        max_frequency,
        min_magnitude,
        max_magnitude,
    )?;

    // Emit config update event
    app.emit(common::events::tuner::CONFIG, updated_config.clone())
        .map_err(|e| TunerError::Emit {
            event: common::events::tuner::CONFIG.to_string(),
            message: e.to_string(),
        })?;

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
    state.reset(&new_config, &tuner_layout);

    // Emit config update event
    app.emit(common::events::tuner::CONFIG, new_config.clone())
        .map_err(|e| TunerError::Emit {
            event: common::events::tuner::CONFIG.to_string(),
            message: e.to_string(),
        })?;

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
