use std::{
    collections::HashMap,
    sync::{mpsc, mpsc::Sender, Arc, Mutex},
};

use crate::rt::TunerRuntime;
use common::{
    error::{InstrumentError, Result},
    tuner::{Config as TunerConfig, SpectrumData},
};
use cpal::traits::HostTrait;
use fundsp::hacker32::prelude::*;

use super::stream::{self, spawn_owned_input_stream, ProdType};
use cpal::traits::DeviceTrait;

use crate::system::input::{FFTAnalyzer, FFT_WINDOW_SIZE};

/// Duration of input ring buffer in milliseconds (approx upper bound).
const BUFFER_DURATION_MS: u64 = 100;
/// Maximum samples kept in rolling buffer (2 seconds @ 48kHz); bounds memory.
const MAX_BUFFER_SAMPLES: usize = 96_000;

/// CPAL-based tuner runtime implementation.
///
/// Responsibilities:
/// - Captures mono input audio (or multi-channel downmixed) via CPAL input stream.
/// - Accumulates samples in an in-memory rolling buffer.
/// - On `poll_spectrum` drains one FFT window of samples into the FFT analyzer,
///   retrieves current spectrum + sensor activations, applies max-hold decay, and
///   returns a `SpectrumData` snapshot.
///
/// MAYA DRY KISS:
/// - Single analyzer instance rebuilt only on start (config changes update the
///   stored config; analyzer hot-reload deferred until needed).
/// - Minimal locking (coarse Mutex) – acceptable for low-frequency polling (20ms).
struct CpalTunerRuntime {
    // Input stream ownership
    input_tx: Mutex<Option<Sender<stream::Control>>>,
    input_thread: Mutex<Option<std::thread::JoinHandle<()>>>,

    // Audio sample rolling buffer (mono f32)
    sample_buffer: Arc<Mutex<Vec<f32>>>,

    // FFT analyzer
    analyzer: Mutex<Option<Box<FFTAnalyzer>>>,

    // Last delivered spectrum (might be useful for callers wanting cached data)
    last_spectrum: Mutex<Option<SpectrumData>>,

    // Config & sensor handling
    config: Mutex<TunerConfig>,
    max_hold: Mutex<Vec<f32>>,
    max_hold_decay: f32,

    // Runtime info
    sample_rate: Mutex<Option<f32>>,
}

impl CpalTunerRuntime {
    fn new(config: TunerConfig) -> Self {
        Self {
            input_tx: Mutex::new(None),
            input_thread: Mutex::new(None),
            sample_buffer: Arc::new(Mutex::new(Vec::new())),
            analyzer: Mutex::new(None),
            last_spectrum: Mutex::new(None),
            config: Mutex::new(config),
            max_hold: Mutex::new(Vec::new()),
            max_hold_decay: 0.95,
            sample_rate: Mutex::new(None),
        }
    }

    fn build_analyzer(&self, config: &TunerConfig, sample_rate: f64) -> Box<FFTAnalyzer> {
        let mut analyzer = Box::new(FFTAnalyzer::new(
            Box::new(pass()),
            FFT_WINDOW_SIZE,
            config.clone(),
            HashMap::new(), // No siren controls for tuner runtime
        ));
        analyzer.set_sample_rate(sample_rate);
        analyzer
    }

    fn ensure_started(&self) -> Result<()> {
        // Already started?
        if self.input_tx.lock().unwrap().is_some() {
            return Ok(());
        }

        // Prepare device
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;

        let default_cfg = input_device
            .default_input_config()
            .map_err(|_| InstrumentError::InputConfigUnavailable)?;
        let stream_cfg: cpal::StreamConfig = default_cfg.clone().into();

        let sr = stream_cfg.sample_rate.0 as f64;
        {
            let cfg = self.config.lock().unwrap().clone();
            let analyzer = self.build_analyzer(&cfg, sr);
            *self.analyzer.lock().unwrap() = Some(analyzer);
            *self.sample_rate.lock().unwrap() = Some(sr as f32);
        }

        // Capacity: derive approximate ring buffer size (in samples) for configured buffer duration.
        let channels = stream_cfg.channels as usize;
        let samples_per_ms = sr / 1000.0;
        let cap_samples = ((samples_per_ms * BUFFER_DURATION_MS as f64).ceil() as usize)
            .saturating_mul(std::cmp::max(1, channels));
        let capacity = cap_samples.next_power_of_two();

        // We'll store samples in an owned Vec (not a fixed ringbuf) – simple & sufficient.
        let sample_buffer = Arc::clone(&self.sample_buffer);

        // Spawn input stream: convert (and down-mix if necessary) -> mono f64 slice -> push into Vec<f32>.
        let (tx, handle) =
            spawn_owned_input_stream(input_device, default_cfg, stream_cfg, move || {
                let buffer = sample_buffer.clone();
                let prod = move |samples: &[f64]| -> usize {
                    // Convert to f32 and append
                    let mut buf = buffer.lock().unwrap();
                    buf.reserve(samples.len());
                    for s in samples {
                        buf.push(*s as f32);
                    }
                    // Trim to rolling max
                    if buf.len() > MAX_BUFFER_SAMPLES {
                        let excess = buf.len() - MAX_BUFFER_SAMPLES;
                        buf.drain(0..excess);
                    }
                    samples.len()
                };
                let boxed: Box<ProdType> = Box::new(prod);
                boxed
            })?;

        *self.input_tx.lock().unwrap() = Some(tx);
        *self.input_thread.lock().unwrap() = Some(handle);

        Ok(())
    }

    fn process_window(&self) -> Option<SpectrumData> {
        let window = FFT_WINDOW_SIZE;
        // Drain a window of samples
        let samples: Vec<f32> = {
            let mut buf = self.sample_buffer.lock().unwrap();
            if buf.len() < window {
                return None;
            }
            buf.drain(0..window).collect()
        };

        let mut analyzer_guard = self.analyzer.lock().unwrap();
        let analyzer = analyzer_guard.as_mut()?;

        // Feed samples sample-by-sample (analyzer expects tick-level input)
        let mut out = [0.0f32];
        for s in &samples {
            analyzer.tick(&[*s], &mut out);
        }

        // Fetch spectrum data
        let (frequencies, magnitudes) = match analyzer.get_spectrum_data() {
            Some(d) => d,
            None => return None,
        };

        // Sensor activations
        let config = self.config.lock().unwrap().clone();
        let activation_map = analyzer.get_sensor_activations();
        let mut sensor_activations = vec![0.0f32; config.sensor_data.len()];
        for (i, s) in config.sensor_data.iter().enumerate() {
            if let Some(v) = activation_map.get(&s.key) {
                sensor_activations[i] = *v;
            }
        }

        // Max-hold update
        let mut max_hold = self.max_hold.lock().unwrap();
        if max_hold.is_empty() {
            *max_hold = magnitudes.clone();
        } else {
            for (i, &cur) in magnitudes.iter().enumerate() {
                if i < max_hold.len() {
                    max_hold[i] *= self.max_hold_decay;
                    if cur > max_hold[i] {
                        max_hold[i] = cur;
                    }
                }
            }
        }

        let sample_rate = self.sample_rate.lock().unwrap().unwrap_or(48_000.0);

        let spectrum = SpectrumData {
            current_magnitudes: magnitudes.clone(),
            max_magnitudes: max_hold.clone(),
            sensor_activations,
            frequencies,
            sample_rate,
            fft_size: window,
        };

        *self.last_spectrum.lock().unwrap() = Some(spectrum.clone());
        Some(spectrum)
    }

    fn shutdown(&self) {
        // Stop input stream
        if let Some(tx) = self.input_tx.lock().unwrap().take() {
            let (ack_tx, ack_rx) = mpsc::channel();
            let _ = tx.send(stream::Control::Shutdown(ack_tx));
            let _ = ack_rx.recv_timeout(std::time::Duration::from_millis(500));
        }

        if let Some(handle) = self.input_thread.lock().unwrap().take() {
            let _ = handle.join();
        }

        // Clear analyzer & buffers
        self.analyzer.lock().unwrap().take();
        self.sample_buffer.lock().unwrap().clear();
        self.last_spectrum.lock().unwrap().take();
        self.max_hold.lock().unwrap().clear();
    }
}

impl TunerRuntime for CpalTunerRuntime {
    fn start(&self, config: &TunerConfig) -> Result<()> {
        {
            // Update stored config before starting
            let mut cfg = self.config.lock().unwrap();
            *cfg = config.clone();
        }
        self.ensure_started()
    }

    fn stop(&self) {
        self.shutdown();
    }

    fn update_config(&self, config: &TunerConfig) {
        let mut cfg = self.config.lock().unwrap();
        *cfg = config.clone();
        // NOTE: Analyzer rebuild deferred (could be added if config materially affects internals).
    }

    fn poll_spectrum(&self) -> Option<SpectrumData> {
        self.process_window()
    }
}

/// Factory for the CPAL tuner runtime.
/// Returns Some boxed runtime if initialization prerequisites succeed,
/// else logs error and returns None (facade falls back to Null).
pub fn make_tuner_runtime() -> Option<Box<dyn TunerRuntime + Send + Sync>> {
    // We do not open devices here to keep construction infallible;
    // actual start() will attempt device/config access and surface errors.
    let default_cfg = TunerConfig::default();
    Some(Box::new(CpalTunerRuntime::new(default_cfg)))
}
