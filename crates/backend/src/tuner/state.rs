use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use common::error::Result;
use common::tuner::{Config, Layout as TunerLayout, SpectrumData};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct TunerState {
    // Persistent config
    pub tuner_config: RwLock<Config>,

    // Derived/ephemeral layout and last instrument layout (for reset)
    pub current_layout: RwLock<Option<TunerLayout>>,
    pub last_instrument_layout: RwLock<Option<common::instrument::Layout>>,

    // Spectrum data buffers
    pub spectrum_buffer: RwLock<Option<SpectrumData>>,
    pub max_hold_buffer: RwLock<Vec<f32>>,
    pub max_hold_decay_rate: RwLock<f32>, // 0.95 = slow decay

    // Analyzer and streaming
    pub tuning_mode: RwLock<bool>,
    #[cfg(feature = "cpal_audio")]
    pub fft_analyzer: RwLock<Option<Box<audio_system::input::analyzer::FFTAnalyzer>>>,
    pub tuner_audio_buffer: Arc<RwLock<Vec<f32>>>,

    // Worker guard
    pub spectrum_streaming: AtomicBool,

    // Platform streaming handles (only with cpal)
    #[cfg(feature = "cpal_audio")]
    pub tuner_input_tx: RwLock<Option<std::sync::mpsc::Sender<crate::instrument::stream::Control>>>,
    #[cfg(feature = "cpal_audio")]
    pub tuner_input_thread: RwLock<Option<std::thread::JoinHandle<()>>>,
}

impl TunerState {
    pub fn new() -> Self {
        Self {
            tuner_config: RwLock::new(Config::default()),
            current_layout: RwLock::new(None),
            last_instrument_layout: RwLock::new(None),
            spectrum_buffer: RwLock::new(None),
            max_hold_buffer: RwLock::new(Vec::new()),
            max_hold_decay_rate: RwLock::new(0.95),
            tuning_mode: RwLock::new(false),
            #[cfg(feature = "cpal_audio")]
            fft_analyzer: RwLock::new(None),
            tuner_audio_buffer: Arc::new(RwLock::new(Vec::new())),
            spectrum_streaming: AtomicBool::new(false),
            #[cfg(feature = "cpal_audio")]
            tuner_input_tx: RwLock::new(None),
            #[cfg(feature = "cpal_audio")]
            tuner_input_thread: RwLock::new(None),
        }
    }

    #[cfg(feature = "cpal_audio")]
    pub fn start_tuner_stream(&self) -> Result<()> {
        use crate::instrument::stream::{spawn_owned_input_stream, ProdType};
        use cpal::traits::{DeviceTrait, HostTrait};
        use fundsp::hacker32::prelude::*;

        // Don't start if already running
        if self.tuner_input_tx.read().is_some() {
            return Ok(());
        }

        *self.tuning_mode.write() = true;

        // Select input device
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .ok_or(common::error::InstrumentError::DeviceUnavailable)?;

        let input_default_cfg = input_device
            .default_input_config()
            .map_err(|_| common::error::InstrumentError::InputConfigUnavailable)?;

        // Prepare analyzer
        let sample_rate = input_default_cfg.sample_rate().0 as f64;
        let tuner_data = self.tuner_config.read().clone();

        let analyzer = Box::new(audio_system::input::analyzer::FFTAnalyzer::new(
            Box::new(pass()),
            audio_system::input::analyzer::FFT_WINDOW_SIZE,
            tuner_data,
            std::collections::HashMap::new(), // No siren controls in tuner mode
        ));

        *self.fft_analyzer.write() = Some(analyzer);

        // Set sample rate for analyzer
        if let Some(ref mut analyzer) = *self.fft_analyzer.write() {
            analyzer.set_sample_rate(sample_rate);
        }

        // Prepare CPAL stream config
        let stream_cfg: cpal::StreamConfig = input_default_cfg.clone().into();

        // Clear and prepare audio buffer
        self.tuner_audio_buffer.write().clear();

        // Clone buffer reference for the input stream
        let audio_buffer = self.tuner_audio_buffer.clone();

        // Create input stream that feeds samples to the buffer
        let (tx, join) = spawn_owned_input_stream(
            input_device,
            input_default_cfg,
            stream_cfg,
            move || {
                let audio_buffer = audio_buffer.clone();
                let mut first_sample = true;
                let prod = move |samples: &[f64]| -> usize {
                    // Log first batch of samples to verify input
                    if first_sample && !samples.is_empty() {
                        let max = samples.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
                        log::info!(
                            "First audio batch: {} samples, max amplitude: {:.6}, first 5 values: {:?}",
                            samples.len(),
                            max,
                            &samples[..std::cmp::Ord::min(samples.len(), 5)]
                        );
                        first_sample = false;
                    }

                    // Convert to f32 and store in buffer
                    let f32_samples: Vec<f32> = samples.iter().map(|&s| s as f32).collect();
                    let len = f32_samples.len();

                    // Store samples in buffer for FFT processing
                    {
                        let mut buffer = audio_buffer.write();
                        let prev_len = buffer.len();
                        buffer.extend(f32_samples);
                        // Keep only last 96000 samples (2 seconds at 48kHz)
                        let buffer_len = buffer.len();
                        if buffer_len > 96000 {
                            buffer.drain(0..buffer_len - 96000);
                        }
                        log::trace!(
                            "Tuner buffer: {} -> {} samples (added {})",
                            prev_len,
                            buffer.len(),
                            len
                        );
                    }

                    len
                };

                let boxed: Box<ProdType> = Box::new(prod);
                boxed
            },
        )?;

        *self.tuner_input_tx.write() = Some(tx);
        *self.tuner_input_thread.write() = Some(join);

        Ok(())
    }

    #[cfg(feature = "cpal_audio")]
    pub fn stop_tuner_stream(&self) {
        use std::sync::mpsc;

        *self.tuning_mode.write() = false;

        // Stop tuner input stream
        if let Some(tx) = self.tuner_input_tx.write().take() {
            let (ack_tx, ack_rx) = mpsc::channel();
            let _ = tx.send(crate::instrument::stream::Control::Shutdown(ack_tx));
            let _ = ack_rx.recv_timeout(std::time::Duration::from_millis(500));
        }

        if let Some(handle) = self.tuner_input_thread.write().take() {
            let _ = handle.join();
        }

        // Clear FFT analyzer
        *self.fft_analyzer.write() = None;

        // Stop spectrum worker
        self.spectrum_streaming.store(false, Ordering::SeqCst);
    }

    #[cfg(feature = "cpal_audio")]
    pub fn start_spectrum_streaming(&self, app: AppHandle) -> Result<()> {
        use fundsp::audiounit::AudioUnit;
        use std::{thread, time::Duration};

        // Avoid starting multiple workers
        if self
            .spectrum_streaming
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        let app = Arc::new(app);
        let state_ptr = self as *const TunerState as usize; // for logs

        thread::spawn(move || {
            log::debug!("Spectrum worker started (state_ptr={state_ptr:x})");
            loop {
                // Check control flag
                {
                    // If disabled externally, exit
                    if let Some(state) = app.try_state::<TunerState>() {
                        if !state.spectrum_streaming.load(Ordering::SeqCst) {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // Process buffered audio through FFT analyzer
                let samples_to_process = {
                    if let Some(state) = app.try_state::<TunerState>() {
                        let mut buffer = state.tuner_audio_buffer.write();
                        if buffer.len() >= 2048 {
                            // FFT window size
                            let samples: Vec<f32> = buffer.drain(0..2048).collect();
                            Some(samples)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                if let Some(samples) = samples_to_process {
                    if let Some(state) = app.try_state::<TunerState>() {
                        let mut fft_analyzer = state.fft_analyzer.write();
                        if let Some(ref mut analyzer) = *fft_analyzer {
                            // The analyzer accumulates internally; tick per-sample is ok given small window.
                            let mut output = vec![0.0f32; 1];
                            for sample in samples {
                                analyzer.tick(&[sample], &mut output);
                            }
                        }
                    }
                }

                // Extract spectrum data and emit
                if let Some(state) = app.try_state::<TunerState>() {
                    if let Some(ref analyzer) = *state.fft_analyzer.read() {
                        if let Some((frequencies, magnitudes)) = analyzer.get_spectrum_data() {
                            // Convert HashMap to Vec for sensor activations
                            let config = state.tuner_config.read();
                            let sensor_map = analyzer.get_sensor_activations();
                            let mut activation_vec = vec![0.0f32; config.sensor_data.len()];
                            for (idx, s) in config.sensor_data.iter().enumerate() {
                                if let Some(v) = sensor_map.get(&s.key) {
                                    activation_vec[idx] = *v;
                                }
                            }

                            // Update max hold buffer
                            {
                                let mut max_hold = state.max_hold_buffer.write();
                                let decay_rate = *state.max_hold_decay_rate.read();
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
                            }

                            let spectrum = SpectrumData {
                                current_magnitudes: magnitudes.clone(),
                                max_magnitudes: state.max_hold_buffer.read().clone(),
                                sensor_activations: activation_vec,
                                frequencies,
                                sample_rate: 48000.0, // TODO: Wire actual capture SR if needed
                                fft_size: 2048,
                            };

                            // Update spectrum buffer
                            *state.spectrum_buffer.write() = Some(spectrum.clone());

                            if let Err(e) = app.emit(common::events::tuner::SPECTRUM_DATA, spectrum)
                            {
                                log::error!("Failed to emit spectrum data: {}", e);
                            }
                        }
                    }
                }

                thread::sleep(Duration::from_millis(20));
            }

            if let Some(state) = app.try_state::<TunerState>() {
                state.spectrum_streaming.store(false, Ordering::SeqCst);
            }
            log::debug!("Spectrum worker stopped (state_ptr={state_ptr:x})");
        });

        Ok(())
    }
}
