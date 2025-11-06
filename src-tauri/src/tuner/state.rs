use std::sync::Arc;
use std::thread;
use std::time::Duration;

use audio_system::{input::analyzer::FFT_WINDOW_SIZE, rt::ExcitementSource};
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
    max_hold_excitements: Arc<RwLock<Vec<f32>>>,
    current_magnitudes: Arc<RwLock<Vec<f32>>>,
    current_excitements: Arc<RwLock<Vec<f32>>>,
    frequencies: Arc<RwLock<Vec<f32>>>,
    spectrum_polling_thread: RwLock<Option<thread::JoinHandle<()>>>,
    audio_probe: RwLock<bool>,
}

impl TunerState {
    pub fn new(app: AppHandle, config: Config) -> Self {
        Self {
            app,
            tuner_config: Arc::new(RwLock::new(config)),
            current_layout: RwLock::new(None),
            max_hold_magnitudes: Arc::new(RwLock::new(Vec::new())),
            max_hold_excitements: Arc::new(RwLock::new(Vec::new())),
            current_magnitudes: Arc::new(RwLock::new(Vec::new())),
            current_excitements: Arc::new(RwLock::new(Vec::new())),
            frequencies: Arc::new(RwLock::new(Vec::new())),
            spectrum_polling_thread: RwLock::new(None),
            audio_probe: RwLock::new(false),
        }
    }

    pub fn tuner_config(&self) -> Config {
        self.tuner_config.read().clone()
    }

    pub fn tuner_layout(&self) -> Option<TunerLayout> {
        self.current_layout.read().iter().copied().next()
    }

    pub fn reset(&self, config: &Config, layout: &TunerLayout) -> Result<()> {
        let instrument = self.app.state::<InstrumentEngine>();
        instrument.update_tuner_config(config)?;

        *self.tuner_config.write() = config.clone();
        *self.current_layout.write() = Some(*layout);

        self.app.emit(common::events::tuner::CONFIG, config)?;

        Ok(())
    }

    pub fn toggle_probe(&self) -> Result<bool> {
        let mut probe = self.audio_probe.write();
        *probe = !*probe;
        let instrument = self.app.state::<InstrumentEngine>();
        if *probe {
            instrument.start_tap_tuner_audio()?;
        } else {
            instrument.stop_tap_tuner_audio()?;
        }
        Ok(*probe)
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

            let instrument = self.app.state::<InstrumentEngine>();
            instrument.update_tuner_config(&new_config)?;

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

        let instrument = self.app.state::<InstrumentEngine>();
        instrument.update_tuner_config(&config)?;

        self.app
            .emit(common::events::tuner::CONFIG, config.clone())?;

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

        if !matches!(instrument.excitement_source(), ExcitementSource::Mic) {
            instrument.set_excitement_source(ExcitementSource::Mic)?;
        }

        if !instrument.playing() {
            instrument.start_tuner_only_stream(&self.tuner_config())?;
        }

        let dur = Duration::from_secs_f32(
            1.0 / (self.tuner_config.read().sample_rate / FFT_WINDOW_SIZE as f32),
        );

        log::debug!("Starting tuner stream with polling interval: {:?}", dur);

        let handle = self.app.clone();
        let max_hold_magnitudes = Arc::clone(&self.max_hold_magnitudes);
        let max_hold_excitements = Arc::clone(&self.max_hold_excitements);
        let current_magnitudes = Arc::clone(&self.current_magnitudes);
        let current_excitements = Arc::clone(&self.current_excitements);
        let frequencies = Arc::clone(&self.frequencies);

        *self.spectrum_polling_thread.write() = Some(thread::spawn(move || loop {
            let instrument_state = handle.state::<InstrumentEngine>();
            while let Some(spectrum) = instrument_state.poll_spectrum() {
                log::trace!("Received spectrum data");

                {
                    let data = spectrum.0;
                    let num_entries = data.len();

                    let mut frequencies = frequencies.write();
                    let mut current_magnitudes = current_magnitudes.write();
                    let mut max_hold_magnitudes = max_hold_magnitudes.write();

                    if num_entries != frequencies.len()
                        || num_entries != current_magnitudes.len()
                        || num_entries != max_hold_magnitudes.len()
                    {
                        // Resize vectors
                        *frequencies = vec![0_f32; num_entries];
                        *current_magnitudes = vec![0_f32; num_entries];
                        *max_hold_magnitudes = vec![0_f32; num_entries];
                    }

                    for ((((freq, mag), frequency), current_magnitude), max_magnitude) in data
                        .iter()
                        .zip(frequencies.iter_mut())
                        .zip(current_magnitudes.iter_mut())
                        .zip(max_hold_magnitudes.iter_mut())
                    {
                        *frequency = *freq;
                        *current_magnitude = *mag;
                        *max_magnitude = f32::min(*mag, *max_magnitude * HOLD_DECAY);
                    }
                }

                {
                    let excitements = instrument_state.poll_excitements();
                    let num_sensors = excitements.len();
                    let mut max_hold_excitements = max_hold_excitements.write();
                    let mut current_excitements = current_excitements.write();
                    if num_sensors != current_excitements.len()
                        || num_sensors != max_hold_excitements.len()
                    {
                        // Resize vectors
                        *current_excitements = vec![0_f32; num_sensors];
                        *max_hold_excitements = vec![0_f32; num_sensors];
                    }

                    for (((_, excitement), current), max_excitement) in excitements
                        .iter()
                        .zip(current_excitements.iter_mut())
                        .zip(max_hold_excitements.iter_mut())
                    {
                        *current = *excitement;
                        *max_excitement = f32::max(*excitement, *max_excitement * HOLD_DECAY);
                    }
                }

                log::trace!("Updated spectrum and excitement data");
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

    pub fn spectrum_data(handle: &AppHandle) -> SpectrumData {
        let state = handle.state::<Self>();

        let max_hold_magnitudes = state.max_hold_magnitudes.read();
        let max_hold_excitements = state.max_hold_excitements.read();
        let current_magnitudes = state.current_magnitudes.read();
        let current_excitements = state.current_excitements.read();
        let frequencies = state.frequencies.read();
        let sample_rate = state.tuner_config.read().sample_rate;

        SpectrumData {
            current_magnitudes: current_magnitudes.clone(),
            max_magnitudes: max_hold_magnitudes.clone(),
            sensor_excitements: current_excitements.clone(),
            max_excitements: max_hold_excitements.clone(),
            frequencies: frequencies.clone(),
            fft_size: FFT_WINDOW_SIZE,
            sample_rate,
        }
    }
}
