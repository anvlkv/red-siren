use std::sync::Arc;
use std::thread;
use std::time::Duration;

use audio_system::{input::analyzer::FFT_WINDOW_SIZE, rt::ActivationSource};
use common::{
    error::{Result, TunerError},
    NodeKeyRegistry,
};
use common::{
    tuner::{Config, Layout as TunerLayout, SpectrumData},
    NodeKey,
};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter, Manager};

// Import InstrumentEngine from parent module
use crate::instrument::InstrumentEngine;

const HOLD_DECAY: f32 = 0.95;

/// Tuner state management using engine with Mic input.
pub struct TunerState {
    app: AppHandle,
    tuner_config: Arc<RwLock<Config>>,
    current_layout: RwLock<Option<TunerLayout>>,
    max_hold_magnitudes: Arc<RwLock<Vec<f32>>>,
    max_hold_activations: Arc<RwLock<Vec<f32>>>,
    current_magnitudes: Arc<RwLock<Vec<f32>>>,
    current_activations: Arc<RwLock<Vec<f32>>>,
    frequencies: Arc<RwLock<Vec<f32>>>,
    spectrum_polling_thread: RwLock<Option<thread::JoinHandle<()>>>,
}

impl TunerState {
    pub fn new(app: AppHandle, config: Config) -> Self {
        Self {
            app,
            tuner_config: Arc::new(RwLock::new(config)),
            current_layout: RwLock::new(None),
            max_hold_magnitudes: Arc::new(RwLock::new(Vec::new())),
            max_hold_activations: Arc::new(RwLock::new(Vec::new())),
            current_magnitudes: Arc::new(RwLock::new(Vec::new())),
            current_activations: Arc::new(RwLock::new(Vec::new())),
            frequencies: Arc::new(RwLock::new(Vec::new())),
            spectrum_polling_thread: RwLock::new(None),
        }
    }

    pub fn tuner_config(&self) -> Config {
        self.tuner_config.read().clone()
    }

    pub fn tuner_layout(&self) -> Option<TunerLayout> {
        self.current_layout.read().iter().copied().next()
    }

    pub fn reset(&self, config: &Config, layout: &TunerLayout) {
        *self.tuner_config.write() = config.clone();
        *self.current_layout.write() = Some(*layout);
    }

    pub fn update_layout(
        &self,
        layout: TunerLayout,
        registry: NodeKeyRegistry,
    ) -> Result<Option<Config>> {
        let current_config = self.tuner_config.read();
        *self.current_layout.write() = Some(layout);

        let has_invalid_keys = !registry
            .has_invalid_keys(
                &current_config
                    .sensor_data
                    .iter()
                    .map(|s| (s.key, ()))
                    .collect(),
            )
            .is_empty();

        let is_empty = current_config.sensor_data.is_empty();
        let wrong_count = current_config.sensor_data.len() != registry.total_keys();

        if is_empty || has_invalid_keys || wrong_count {
            // Preserve old sensor frequency settings if possible
            let old_sensor_settings = {
                current_config
                    .sensor_data
                    .iter()
                    .map(|s| {
                        (
                            s.key,
                            (
                                s.min_frequency,
                                s.max_frequency,
                                s.min_magnitude,
                                s.max_magnitude,
                            ),
                        )
                    })
                    .collect::<std::collections::HashMap<NodeKey, (f32, f32, f32, f32)>>()
            };

            drop(current_config); // Release read lock before acquiring write lock

            let instrument_state = self.app.state::<InstrumentEngine>();
            let sample_rate = instrument_state.sample_rate() as f32;

            let mut new_config: Config = Config::new(layout, sample_rate, registry);

            // Restore preserved settings where possible
            for sensor in &mut new_config.sensor_data {
                if let Some((min_freq, max_freq, min_mag, max_mag)) =
                    old_sensor_settings.get(&sensor.key)
                {
                    sensor.min_frequency = *min_freq;
                    sensor.max_frequency = *max_freq;
                    sensor.min_magnitude = *min_mag;
                    sensor.max_magnitude = *max_mag;
                }
            }

            *self.tuner_config.write() = new_config.clone();

            self.app
                .emit(common::events::tuner::CONFIG, new_config.clone())?;

            Ok(Some(new_config))
        } else {
            Ok(None)
        }
    }

    pub fn update_sensor_valuess(
        &self,
        key: NodeKey,
        min_frequency: f32,
        max_frequency: f32,
        min_magnitude: f32,
        max_magnitude: f32,
    ) -> Result<Config> {
        let mut config = self.tuner_config.write();

        if let Some(sensor) = config.sensor_data.iter_mut().find(|s| s.key == key) {
            sensor.min_frequency = min_frequency;
            sensor.max_frequency = max_frequency;
            sensor.min_magnitude = min_magnitude;
            sensor.max_magnitude = max_magnitude;
        } else {
            return Err(
                TunerError::MissingParameter(format!("sensor for node key: {:?}", key)).into(),
            );
        }

        Ok(config.clone())
    }

    /// Start tuner streaming & spectrum polling.
    /// Idempotent: repeated calls while running are ignored.
    pub fn start_tuner_stream(&self) -> Result<()> {
        // Check if already running
        if self.spectrum_polling_thread.read().is_some() {
            log::info!("Tuner stream already running, ignoring start request");
            return Ok(());
        }

        let instrument = self.app.state::<InstrumentEngine>();

        if !matches!(instrument.activation_source(), ActivationSource::Mic) {
            instrument.set_activation_source(ActivationSource::Mic)?;
        }

        if !instrument.playing() {
            instrument.start_tuner_only_stream(&self.tuner_config())?;
        }

        let dur =
            Duration::from_secs_f32(FFT_WINDOW_SIZE as f32 / self.tuner_config.read().sample_rate);

        let handle = self.app.clone();
        let max_hold_magnitudes = Arc::clone(&self.max_hold_magnitudes);
        let max_hold_activations = Arc::clone(&self.max_hold_activations);
        let current_magnitudes = Arc::clone(&self.current_magnitudes);
        let current_activations = Arc::clone(&self.current_activations);
        let frequencies = Arc::clone(&self.frequencies);

        *self.spectrum_polling_thread.write() = Some(thread::spawn(move || loop {
            let instrument_state = handle.state::<InstrumentEngine>();
            if let Some(spectrum) = instrument_state.poll_spectrum() {
                let data = spectrum.data();

                let mut frequencies = frequencies.write();
                let mut current_magnitudes = current_magnitudes.write();
                let mut max_hold_magnitudes = max_hold_magnitudes.write();

                if data.len() != frequencies.len()
                    || data.len() != current_magnitudes.len()
                    || data.len() != max_hold_magnitudes.len()
                {
                    // Resize vectors
                    *frequencies = vec![0.0; data.len()];
                    *current_magnitudes = vec![0.0; data.len()];
                    *max_hold_magnitudes = vec![0.0; data.len()];
                }

                for (at, (freq, mag)) in data.iter().enumerate() {
                    frequencies[at] = freq.val();
                    current_magnitudes[at] = mag.val();
                    // Update max hold with decay
                    max_hold_magnitudes[at] *= HOLD_DECAY;
                    if mag.val() > max_hold_magnitudes[at] {
                        max_hold_magnitudes[at] = mag.val();
                    }
                }

                let activations = instrument_state.snapshot_all_activation_snoops();
                let num_sensors = activations.len();
                let mut max_hold_activations = max_hold_activations.write();
                let mut current_activations = current_activations.write();
                if num_sensors != current_activations.len()
                    || num_sensors != max_hold_activations.len()
                {
                    // Resize vectors
                    *current_activations = vec![0.0; num_sensors];
                    *max_hold_activations = vec![0.0; num_sensors];
                }

                for (at, (_, _, activation)) in activations.iter().enumerate() {
                    let activation = activation.iter().sum::<f32>() / activation.len() as f32;
                    current_activations[at] = activation;
                    // Update max hold with decay
                    max_hold_activations[at] *= HOLD_DECAY;
                    if activation > max_hold_activations[at] {
                        max_hold_activations[at] = activation;
                    }
                }
            }

            thread::sleep(dur);
        }));

        Ok(())
    }

    /// Stop tuner streaming & polling.
    pub fn stop_tuner_stream(&self) -> Result<()> {
        log::info!("Stopping tuner stream");

        if let Some(handle) = self.spectrum_polling_thread.write().take() {
            drop(handle);
        }

        let instrument = self.app.state::<InstrumentEngine>();
        if !instrument.playing() {
            instrument.stop_tuner_only_stream()?;
        }

        log::info!("Tuner stream stopped");

        Ok(())
    }

    pub fn spectrum_data(&self) -> SpectrumData {
        let max_hold_magnitudes = self.max_hold_magnitudes.read();
        let max_hold_activations = self.max_hold_activations.read();
        let current_magnitudes = self.current_magnitudes.read();
        let current_activations = self.current_activations.read();
        let frequencies = self.frequencies.read();
        let sample_rate = self.tuner_config.read().sample_rate;

        SpectrumData {
            current_magnitudes: current_magnitudes.clone(),
            max_magnitudes: max_hold_magnitudes.clone(),
            sensor_activations: current_activations.clone(),
            max_activations: max_hold_activations.clone(),
            frequencies: frequencies.clone(),
            fft_size: FFT_WINDOW_SIZE,
            sample_rate,
        }
    }
}
