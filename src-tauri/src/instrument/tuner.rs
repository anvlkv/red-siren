//! Tuner module for spectrum analysis and sensor configuration

use common::error::Result;
use common::tuner::{Config, SpectrumData};
use fundsp::hacker32::AudioUnit;
use tauri::{AppHandle, Emitter, State};

use super::InstrumentState;

/// Get current tuner configuration
#[tauri::command]
pub fn tuner_config(state: State<'_, InstrumentState>) -> Result<Config> {
    let config = state.inner.tuner_data.read().clone();
    Ok(config)
}

/// Get current spectrum data with max hold tracking
#[tauri::command]
pub fn tuner_spectrum_data(state: State<'_, InstrumentState>) -> Result<Option<SpectrumData>> {
    // Get current spectrum buffer
    let spectrum_data = state.inner.spectrum_buffer.read().clone();

    // If we have data, update max hold buffer
    if let Some(ref data) = spectrum_data {
        let mut max_hold = state.inner.max_hold_buffer.write();
        let decay_rate = *state.inner.max_hold_decay_rate.read();

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

/// Update sensor configuration
#[tauri::command]
pub fn tuner_update_sensor(
    state: State<'_, InstrumentState>,
    app: AppHandle,
    index: usize,
    min_frequency: f32,
    max_frequency: f32,
    min_magnitude: f32,
    max_magnitude: f32,
) -> Result<()> {
    // Update tuner config
    {
        let mut config = state.inner.tuner_data.write();

        // Validate index
        if index >= config.sensor_data.len() {
            return Err(common::error::TunerError::InvalidSensorIndex { index }.into());
        }

        // Update sensor data
        config.sensor_data[index].min_frequency = min_frequency;
        config.sensor_data[index].max_frequency = max_frequency;
        config.sensor_data[index].min_magnitude = min_magnitude;
        config.sensor_data[index].max_magnitude = max_magnitude;

        // Emit config update event
        app.emit(common::events::tuner::CONFIG, config.clone())
            .map_err(|e| common::error::TunerError::Emit {
                event: common::events::tuner::CONFIG.to_string(),
                message: e.to_string(),
            })?;
    }

    // TODO: Apply changes to FFT analyzer
    // This will need to update the running analyzer with new sensor ranges

    Ok(())
}

/// Reset tuner config to defaults from instrument layout
#[tauri::command]
pub fn tuner_reset_config(state: State<'_, InstrumentState>, app: AppHandle) -> Result<()> {
    // Get current instrument layout
    let layout = state.inner.layout();

    // Generate default tuner config from layout
    let new_config: Config = layout.into();

    // Update tuner config
    {
        let mut config = state.inner.tuner_data.write();
        *config = new_config.clone();
    }

    // Clear max hold buffer
    {
        let mut max_hold = state.inner.max_hold_buffer.write();
        max_hold.clear();
    }

    // Emit config update event
    app.emit(common::events::tuner::CONFIG, new_config)
        .map_err(|e| common::error::TunerError::Emit {
            event: common::events::tuner::CONFIG.to_string(),
            message: e.to_string(),
        })?;

    // TODO: Apply changes to FFT analyzer

    Ok(())
}

/// Start tuner input stream for spectrum analysis
#[tauri::command]
pub fn tuner_start_stream(state: State<'_, InstrumentState>, app: AppHandle) -> Result<()> {
    state.inner.start_tuner_stream(app.clone())?;
    // Also start the spectrum streaming
    start_spectrum_streaming(&state, app)?;
    Ok(())
}

/// Stop tuner input stream
#[tauri::command]
pub fn tuner_stop_stream(state: State<'_, InstrumentState>) -> Result<()> {
    state.inner.stop_tuner_stream();
    Ok(())
}

/// Start spectrum data streaming
pub fn start_spectrum_streaming(_state: &InstrumentState, app: AppHandle) -> Result<()> {
    // This will be called from the engine when starting
    // It should periodically extract FFT data and emit spectrum events

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use tauri::{Emitter, Manager};

    // Create a flag to control the streaming thread
    static STREAMING_ACTIVE: AtomicBool = AtomicBool::new(false);
    STREAMING_ACTIVE.store(true, Ordering::SeqCst);

    let app = Arc::new(app);

    // Start spectrum update thread
    thread::spawn(move || {
        loop {
            // Check if we should stop
            if !STREAMING_ACTIVE.load(Ordering::SeqCst) {
                break;
            }

            // Get state from app handle
            let spectrum_data = {
                if let Some(state) = app.try_state::<InstrumentState>() {
                    // Check if we should continue (tuning mode or playing)
                    let tuning = *state.inner.tuning_mode.read();
                    let playing = *state.inner.playing.read();

                    if !tuning && !playing {
                        STREAMING_ACTIVE.store(false, Ordering::SeqCst);
                        break;
                    }

                    // Process buffered audio through FFT analyzer
                    let samples_to_process = {
                        let mut buffer = state.inner.tuner_audio_buffer.write();
                        if buffer.len() >= 2048 {
                            // FFT window size
                            // Take the first 2048 samples for FFT processing
                            let samples: Vec<f32> = buffer.drain(0..2048).collect();
                            Some(samples)
                        } else {
                            None
                        }
                    };

                    // Process samples through analyzer if we have enough
                    if let Some(samples) = samples_to_process {
                        log::trace!("Processing {} samples through FFT analyzer", samples.len());
                        let mut fft_analyzer = state.inner.fft_analyzer.write();
                        if let Some(ref mut analyzer) = *fft_analyzer {
                            // Process each sample through the analyzer
                            // The analyzer accumulates samples internally and performs FFT when ready
                            let mut output = vec![0.0f32; 1];
                            for sample in samples {
                                analyzer.tick(&[sample], &mut output);
                            }
                            log::trace!("FFT analyzer processed samples");
                        } else {
                            log::warn!("No FFT analyzer available for processing");
                        }
                    } else {
                        log::trace!(
                            "Not enough samples for FFT processing, buffer size: {}",
                            state.inner.tuner_audio_buffer.read().len()
                        );
                    }

                    // Extract spectrum data from FFT analyzer
                    if let Some(ref analyzer) = *state.inner.fft_analyzer.read() {
                        if let Some((frequencies, magnitudes)) = analyzer.get_spectrum_data() {
                            log::trace!(
                                "Got spectrum data: {} frequencies, {} magnitudes",
                                frequencies.len(),
                                magnitudes.len()
                            );
                            let sensor_activations = analyzer.get_sensor_activations();

                            // Convert HashMap to Vec for sensor activations
                            let config = state.inner.tuner_data.read();
                            let mut activation_vec = vec![0.0f32; config.sensor_data.len()];
                            for (key, value) in sensor_activations {
                                // Find index of this key in sensor_data
                                if let Some(index) =
                                    config.sensor_data.iter().position(|s| s.key == key)
                                {
                                    if index < activation_vec.len() {
                                        activation_vec[index] = value;
                                    }
                                }
                            }

                            // Update max hold buffer
                            let mut max_hold = state.inner.max_hold_buffer.write();
                            let decay_rate = *state.inner.max_hold_decay_rate.read();

                            if max_hold.is_empty() {
                                *max_hold = magnitudes.clone();
                            } else {
                                for (i, &current) in magnitudes.iter().enumerate() {
                                    if i < max_hold.len() {
                                        max_hold[i] *= decay_rate;
                                        if current > max_hold[i] {
                                            max_hold[i] = current;
                                        }
                                    }
                                }
                            }

                            let spectrum = SpectrumData {
                                current_magnitudes: magnitudes.clone(),
                                max_magnitudes: max_hold.clone(),
                                sensor_activations: activation_vec,
                                frequencies,
                                sample_rate: 48000.0, // TODO: Get actual sample rate from state
                                fft_size: 2048,
                            };

                            // Update spectrum buffer
                            *state.inner.spectrum_buffer.write() = Some(spectrum.clone());

                            Some(spectrum)
                        } else {
                            log::trace!("No spectrum data available from analyzer");
                            None
                        }
                    } else {
                        log::trace!("No FFT analyzer available for spectrum extraction");
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(data) = spectrum_data {
                // Emit spectrum data event
                log::trace!(
                    "Emitting spectrum data with {} sensor activations",
                    data.sensor_activations.len()
                );
                if let Err(e) = app.emit(common::events::tuner::SPECTRUM_DATA, data) {
                    log::error!("Failed to emit spectrum data: {}", e);
                }
            } else {
                log::trace!("No spectrum data to emit");
            }

            // Update rate: faster to process audio buffer
            thread::sleep(Duration::from_millis(20));
        }
    });

    Ok(())
}
