use std::sync::Arc;

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

// Import InstrumentState from parent module
use crate::instrument::InstrumentState;

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
    tuner_stream_active: RwLock<bool>,
    probe_active: RwLock<bool>,
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
            tuner_stream_active: RwLock::new(false),
            probe_active: RwLock::new(false),
        }
    }

    pub fn tuner_config(&self) -> Config {
        self.tuner_config.read().clone()
    }

    pub fn tuner_layout(&self) -> Option<TunerLayout> {
        self.current_layout.read().iter().copied().next()
    }

    pub fn reset(&self, config: &Config, layout: &TunerLayout) -> Result<()> {
        let instrument = self.app.state::<InstrumentState>();
        instrument.update_tuner_config(config)?;

        *self.tuner_config.write() = config.clone();
        *self.current_layout.write() = Some(*layout);

        self.app
            .emit(common::events::tuner::CONFIG, config.clone())?;

        super::setup::save_tuner_config(&self.app, config.clone())?;

        self.app
            .emit(common::events::tuner::CONSTRAINTS, config.constraints())?;

        Ok(())
    }

    pub fn toggle_probe(&self) -> Result<bool> {
        let instrument = self.app.state::<InstrumentState>();
        let mut probe = self.probe_active.write();
        let desired = !*probe;
        if desired {
            instrument.start_tap_tuner_audio()?;
        } else {
            instrument.stop_tap_tuner_audio()?;
        }
        *probe = desired;
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
            let instrument_state = self.app.state::<InstrumentState>();
            let sample_rate = instrument_state.sample_rate() as f32;

            let new_config: Config = Config::new_from_previous(
                layout,
                sample_rate,
                audio_system::FFT_WINDOW_SIZE,
                registry,
                &current_config,
            );

            let instrument = self.app.state::<InstrumentState>();
            instrument.update_tuner_config(&new_config)?;

            drop(current_config); // Release read lock before acquiring write lock

            *self.tuner_config.write() = new_config.clone();

            self.app
                .emit(common::events::tuner::CONFIG, new_config.clone())?;

            super::setup::save_tuner_config(&self.app, new_config.clone())?;

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
        // Update under a short-lived write lock, then snapshot
        let updated = {
            let mut config = self.tuner_config.write();

            let (min_lim, max_lim) = config.frequency_range_limits();

            if let Some(sensor) = config.sensor_data.iter_mut().find(|s| s.key == key) {
                sensor.min_frequency = min_frequency.clamp(min_lim, max_lim);
                sensor.max_frequency = max_frequency.clamp(min_lim, max_lim);
                sensor.min_magnitude = min_magnitude;
                sensor.max_magnitude = max_magnitude;
                config.clone()
            } else {
                return Err(TunerError::MissingParameter(format!(
                    "sensor for node key: {:?}",
                    key
                ))
                .into());
            }
        };

        // External operations without holding the lock
        let instrument = self.app.state::<InstrumentState>();
        instrument.update_tuner_config(&updated)?;

        self.app
            .emit(common::events::tuner::CONFIG, updated.clone())?;

        super::setup::save_tuner_config(&self.app, updated.clone())?;

        Ok(updated)
    }

    pub fn update_range(
        &self,
        min_frequency: Option<f32>,
        max_frequency: Option<f32>,
    ) -> Result<()> {
        // Apply update under lock and snapshot
        let updated = {
            let mut config = self.tuner_config.write();
            config.update_frequency_range(min_frequency, max_frequency);
            config.clone()
        };

        let instrument = self.app.state::<InstrumentState>();
        instrument.update_tuner_config(&updated)?;

        self.app
            .emit(common::events::tuner::CONFIG, updated.clone())?;

        self.app
            .emit(common::events::tuner::CONSTRAINTS, updated.constraints())?;

        super::setup::save_tuner_config(&self.app, updated.clone())?;

        Ok(())
    }

    pub fn update_ny_threshold(&self, ny_threshold: f32) -> Result<()> {
        // Update under lock and snapshot
        let updated = {
            let mut config = self.tuner_config.write();
            config.ny_threshold = ny_threshold;
            config.clone()
        };

        let instrument = self.app.state::<InstrumentState>();
        instrument.update_tuner_config(&updated)?;

        self.app
            .emit(common::events::tuner::CONSTRAINTS, updated.constraints())?;

        super::setup::save_tuner_config(&self.app, updated.clone())?;

        Ok(())
    }

    pub fn update_wet_ratio(&self, wet_ratio: f32) -> Result<()> {
        // Update under lock and snapshot
        let updated = {
            let mut config = self.tuner_config.write();
            config.ny_wet_ratio = wet_ratio;
            config.clone()
        };

        let instrument = self.app.state::<InstrumentState>();
        instrument.update_tuner_config(&updated)?;

        self.app
            .emit(common::events::tuner::CONSTRAINTS, updated.constraints())?;

        super::setup::save_tuner_config(&self.app, updated.clone())?;

        Ok(())
    }

    /// Start tuner streaming.
    /// Idempotent: repeated calls while running are ignored.
    ///
    /// Hardened: if playback is still fading out when switching to tuner,
    /// retry starting tuner-only stream for a short period before falling back
    /// to using the existing stream without owning it.
    pub fn start_tuner_stream(&self) -> Result<()> {
        let instrument = self.app.state::<InstrumentState>();

        log::debug!(
            "tuner.start_tuner_stream: begin (playing={}, src={:?})",
            instrument.playing(),
            instrument.excitement_source()
        );

        // Ensure Mic is the excitement source for tuner use
        if !matches!(instrument.excitement_source(), ExcitementSource::Mic) {
            log::debug!("tuner.start_tuner_stream: switching excitement source to Mic");
            instrument.set_excitement_source(ExcitementSource::Mic)?;
        }

        // If not already playing, start a dedicated tuner-only stream and mark active.
        if !instrument.playing() {
            log::debug!(
                "tuner.start_tuner_stream: controller not playing; starting tuner-only stream now"
            );
            instrument.start_tuner_only_stream(&self.tuner_config())?;
            *self.tuner_stream_active.write() = true;
            log::debug!("tuner.start_tuner_stream: started tuner-only stream (owned=true)");
        } else {
            // Already playing (likely in fade) — attempt short retries to take over with tuner-only.
            log::debug!("tuner.start_tuner_stream: controller playing; attempting short retries to start tuner-only after fade-out");
            let mut owned = false;
            for attempt in 1..=10 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                let still_playing = instrument.playing();
                log::trace!(
                    "tuner.start_tuner_stream: retry {attempt}/10 (playing={still_playing})"
                );
                if !still_playing {
                    match instrument.start_tuner_only_stream(&self.tuner_config()) {
                        Ok(()) => {
                            owned = true;
                            log::debug!(
                                "tuner.start_tuner_stream: took over with tuner-only on retry {attempt}"
                            );
                            break;
                        }
                        Err(e) => {
                            log::warn!(
                                "tuner.start_tuner_stream: failed to start tuner-only on retry {attempt}: {e}"
                            );
                        }
                    }
                }
            }
            *self.tuner_stream_active.write() = owned;
            if !owned {
                log::warn!(
                    "tuner.start_tuner_stream: could not take ownership; using existing stream (owned=false)"
                );
            }
        }

        log::debug!(
            "tuner.start_tuner_stream: done (owned={}, playing={}, src={:?})",
            *self.tuner_stream_active.read(),
            instrument.playing(),
            instrument.excitement_source()
        );

        self.app
            .emit(common::instrument::events::LAYOUT, instrument.layout())
            .map_err(|e| TunerError::Emit {
                event: common::instrument::events::LAYOUT.to_string(),
                message: e.to_string(),
            })?;

        self.app
            .emit(
                common::tuner::events::LAYOUT,
                self.tuner_layout().unwrap_or_default(),
            )
            .map_err(|e| TunerError::Emit {
                event: common::tuner::events::LAYOUT.to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    /// Stop tuner streaming.
    pub fn stop_tuner_stream(&self) -> Result<()> {
        log::info!("Stopping tuner stream");

        let instrument = self.app.state::<InstrumentState>();
        // Only stop if we started a dedicated tuner-only stream.
        let owned = *self.tuner_stream_active.read();
        if owned {
            if instrument.playing() {
                instrument.stop_tuner_only_stream()?;
            }
            *self.tuner_stream_active.write() = false;
        }

        log::info!("Tuner stream stopped");

        Ok(())
    }

    pub fn spectrum_data(handle: &AppHandle) -> SpectrumData {
        let state = handle.state::<Self>();
        let instrument_state = handle.state::<InstrumentState>();

        // Poll a spectrum frame if available and update cached vectors
        if let Some(spectrum) = instrument_state.poll_spectrum() {
            let data = spectrum.0;
            let num_entries = data.len();

            {
                let mut frequencies = state.frequencies.write();
                let mut current_magnitudes = state.current_magnitudes.write();
                let mut max_hold_magnitudes = state.max_hold_magnitudes.write();

                if num_entries != frequencies.len()
                    || num_entries != current_magnitudes.len()
                    || num_entries != max_hold_magnitudes.len()
                {
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
                    *max_magnitude = f32::max(*mag, *max_magnitude * HOLD_DECAY);
                }
            }
        }

        // Poll excitements and update cached vectors
        {
            let excitements = instrument_state.poll_excitements();
            let num_sensors = excitements.len();
            let mut max_hold_excitements = state.max_hold_excitements.write();
            let mut current_excitements = state.current_excitements.write();
            if num_sensors != current_excitements.len() || num_sensors != max_hold_excitements.len()
            {
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

        let sample_rate = { state.tuner_config.read().sample_rate };

        // Clone into locals to drop read locks before constructing the return value
        let current_magnitudes = state.current_magnitudes.read().clone();
        let max_magnitudes = state.max_hold_magnitudes.read().clone();
        let sensor_excitements = state.current_excitements.read().clone();
        let max_excitements = state.max_hold_excitements.read().clone();
        let frequencies = state.frequencies.read().clone();

        SpectrumData {
            current_magnitudes,
            max_magnitudes,
            sensor_excitements,
            max_excitements,
            frequencies,
            fft_size: FFT_WINDOW_SIZE,
            sample_rate,
        }
    }

    pub fn snapshot_input_snoop(&self) -> Result<Vec<f32>> {
        let instrument = self.app.state::<InstrumentState>();
        Ok(instrument.snapshot_input_snoop())
    }
}
