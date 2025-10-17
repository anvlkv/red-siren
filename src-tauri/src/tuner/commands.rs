use common::error::{Result, TunerError};
use common::tuner::{Config, Layout as TunerLayout, SpectrumData};
use tauri::{AppHandle, Emitter, State};

use super::TunerState;

#[tauri::command]
pub fn tuner_config(state: State<'_, TunerState>) -> Result<Config> {
    Ok(state.tuner_config.read().clone())
}

#[tauri::command]
pub fn tuner_layout(state: State<'_, TunerState>) -> Result<TunerLayout> {
    if let Some(l) = state.current_layout.read().as_ref() {
        return Ok(*l);
    }
    if let Some(inst) = state.last_instrument_layout.read().as_ref() {
        let t: TunerLayout = (*inst).into();
        *state.current_layout.write() = Some(t);
        return Ok(t);
    }
    // Fallback to default
    Ok(TunerLayout::default())
}

#[tauri::command]
pub fn tuner_spectrum_data(state: State<'_, TunerState>) -> Result<Option<SpectrumData>> {
    // Get current spectrum buffer
    let spectrum_data = state.spectrum_buffer.read().clone();

    // If we have data, update max hold buffer
    if let Some(ref data) = spectrum_data {
        let mut max_hold = state.max_hold_buffer.write();
        let decay_rate = *state.max_hold_decay_rate.read();

        // Initialize max hold if empty
        if max_hold.is_empty() {
            *max_hold = data.current_magnitudes.clone();
        } else {
            // Update max hold with decay
            for (i, &current) in data.current_magnitudes.iter().enumerate() {
                if i < max_hold.len() {
                    // Apply decay
                    max_hold[i] *= decay_rate;
                    // Update if current is higher
                    if current > max_hold[i] {
                        max_hold[i] = current;
                    }
                }
            }
        }
    }

    Ok(spectrum_data)
}

#[tauri::command]
pub fn tuner_update_sensor(
    state: State<'_, TunerState>,
    app: AppHandle,
    index: usize,
    min_frequency: f32,
    max_frequency: f32,
    min_magnitude: f32,
    max_magnitude: f32,
) -> Result<()> {
    // Update tuner config
    {
        let mut config = state.tuner_config.write();

        // Validate index
        if index >= config.sensor_data.len() {
            return Err(TunerError::InvalidSensorIndex { index }.into());
        }

        // Update sensor data
        config.sensor_data[index].min_frequency = min_frequency;
        config.sensor_data[index].max_frequency = max_frequency;
        config.sensor_data[index].min_magnitude = min_magnitude;
        config.sensor_data[index].max_magnitude = max_magnitude;

        // Emit config update event
        app.emit(common::events::tuner::CONFIG, config.clone())
            .map_err(|e| TunerError::Emit {
                event: common::events::tuner::CONFIG.to_string(),
                message: e.to_string(),
            })?;

        // Update runtime with new config if it's running
        state.update_runtime_config(&config);
    }

    Ok(())
}

#[tauri::command]
pub fn tuner_reset_config(state: State<'_, TunerState>, app: AppHandle) -> Result<()> {
    // Use last instrument layout if available; else fallback to default
    let inst_layout = state.last_instrument_layout.read().unwrap_or_default();

    // Generate default tuner config from layout
    let new_config: Config = inst_layout.into();

    // Update tuner config
    {
        let mut config = state.tuner_config.write();
        *config = new_config.clone();
    }

    // Clear max hold buffer
    state.max_hold_buffer.write().clear();

    // Emit config update event
    app.emit(common::events::tuner::CONFIG, new_config.clone())
        .map_err(|e| TunerError::Emit {
            event: common::events::tuner::CONFIG.to_string(),
            message: e.to_string(),
        })?;

    // Update runtime with new config if it's running
    state.update_runtime_config(&new_config);

    Ok(())
}

#[tauri::command]
pub fn tuner_start_stream(state: State<'_, TunerState>, app: AppHandle) -> Result<()> {
    state.start_tuner_stream(app)?;
    Ok(())
}

#[tauri::command]
pub fn tuner_stop_stream(state: State<'_, TunerState>) -> Result<()> {
    state.stop_tuner_stream();
    Ok(())
}
