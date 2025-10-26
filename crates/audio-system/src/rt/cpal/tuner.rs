#![allow(clippy::manual_is_multiple_of)]

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
    mpsc::Sender,
    Arc, Mutex,
};

use crate::rt::TunerRuntime;
use common::{
    error::{InstrumentError, Result},
    tuner::{Config as TunerConfig, SpectrumData},
};
use cpal::traits::HostTrait;
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use super::stream::{self, spawn_owned_input_stream, ProdType};
use cpal::traits::DeviceTrait;

use crate::system::input::{FFTAnalyzer, FFT_WINDOW_SIZE};

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
    activation_max_hold: Mutex<Vec<f32>>,
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
            activation_max_hold: Mutex::new(Vec::new()),
            max_hold_decay: 0.95,
            sample_rate: Mutex::new(None),
        }
    }

    fn build_analyzer(&self, config: &TunerConfig, sample_rate: f64) -> Box<FFTAnalyzer> {
        let preamp = crate::input::preamp::create_sensors_preamp();

        let mut analyzer = Box::new(FFTAnalyzer::new_tuner_stub(
            Box::new(preamp),
            FFT_WINDOW_SIZE,
            config.clone(),
        ));
        analyzer.set_sample_rate(sample_rate);
        analyzer
    }

    fn ensure_started(&self) -> Result<()> {
        log::trace!("tuner: ensure_started called");
        // Already started?
        if self.input_tx.lock().unwrap().is_some() {
            log::trace!("tuner: already started");
            return Ok(());
        }

        // Prepare device
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;
        let device_name = input_device.name().unwrap_or_else(|_| "<unknown>".into());

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
        log::trace!(
            "tuner: starting input stream device='{}' sample_rate={} channels={}",
            device_name,
            sr,
            stream_cfg.channels
        );

        // We'll store samples in an owned Vec (not a fixed ringbuf) – simple & sufficient / simplified (removed unused capacity calc).
        let sample_buffer = Arc::clone(&self.sample_buffer);
        let pushed_counter = Arc::new(AtomicUsize::new(0));

        // Spawn input stream: convert (and down-mix if necessary) -> mono f64 slice -> push into Vec<f32>.
        let (tx, handle) = spawn_owned_input_stream(input_device, default_cfg, stream_cfg, {
            let pushed_counter = pushed_counter.clone();
            move || {
                let buffer = sample_buffer.clone();
                let pushed_counter = pushed_counter.clone();
                let prod = move |samples: &[f64]| -> usize {
                    // Convert to f32 and append
                    let mut buf = buffer.lock().unwrap();
                    for s in samples {
                        buf.push(*s as f32);
                    }
                    // Trim to rolling max
                    if buf.len() > MAX_BUFFER_SAMPLES {
                        let excess = buf.len() - MAX_BUFFER_SAMPLES;
                        buf.drain(0..excess);
                        log::trace!(
                            "tuner: trimmed {} samples (buffer_len={})",
                            excess,
                            buf.len()
                        );
                    }
                    let total =
                        pushed_counter.fetch_add(samples.len(), Ordering::Relaxed) + samples.len();
                    if total % FFT_WINDOW_SIZE == 0 {
                        log::trace!(
                            "tuner: ingested {} samples (buffer_len={})",
                            total,
                            buf.len()
                        );
                    }
                    samples.len()
                };
                let boxed: Box<ProdType> = Box::new(prod);
                boxed
            }
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
            let available = buf.len();
            if available < window {
                log::trace!(
                    "tuner: process_window insufficient samples (have {}, need {})",
                    available,
                    window
                );
                return None;
            }
            let drained: Vec<f32> = buf.drain(0..window).collect();
            if log::log_enabled!(log::Level::Trace) {
                log::trace!(
                    "tuner: process_window drained {} samples (remaining {})",
                    drained.len(),
                    buf.len()
                );
            }
            drained
        };

        let mut analyzer_guard = self.analyzer.lock().unwrap();
        let analyzer = analyzer_guard.as_mut()?;

        // Analyze full window (preamp + FFT) using BigBlockAdapter
        analyzer.analyze_window(&samples);
        log::trace!("tuner: analyzed window of {} samples", samples.len());

        // Fetch spectrum data
        let (frequencies, magnitudes) = match analyzer.get_spectrum_data() {
            Some(data) => data,
            None => {
                log::trace!("tuner: analyzer returned no spectrum yet");
                return None;
            }
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

        // Max-hold update for magnitudes (dB)
        let mut max_hold = self.max_hold.lock().unwrap();
        if max_hold.is_empty() {
            *max_hold = magnitudes.clone();
        } else {
            for (i, &cur) in magnitudes.iter().enumerate() {
                if i < max_hold.len() {
                    // Decay and compare in linear domain (dB -> linear -> decay/compare -> dB)
                    let cur_lin = (10.0f32).powf(cur / 20.0);
                    let mut prev_lin = (10.0f32).powf(max_hold[i] / 20.0);
                    prev_lin *= self.max_hold_decay;
                    let new_lin = prev_lin.max(cur_lin);
                    max_hold[i] = 20.0 * new_lin.log10();
                }
            }
        }

        // Max-hold update for sensor activations (0..1)
        let mut activation_max = self.activation_max_hold.lock().unwrap();
        if activation_max.len() != sensor_activations.len() {
            *activation_max = sensor_activations.clone();
        } else {
            for (i, &cur) in sensor_activations.iter().enumerate() {
                let prev = activation_max[i] * self.max_hold_decay;
                activation_max[i] = prev.max(cur);
            }
        }
        let max_activations = activation_max.clone();

        let sample_rate = self.sample_rate.lock().unwrap().unwrap_or(48_000.0);

        let spectrum = SpectrumData {
            current_magnitudes: magnitudes.clone(),
            max_magnitudes: max_hold.clone(),
            sensor_activations,
            max_activations,
            frequencies,
            sample_rate,
            fft_size: window,
        };

        if log::log_enabled!(log::Level::Trace) {
            let preview: Vec<f32> = spectrum
                .current_magnitudes
                .iter()
                .cloned()
                .take(8)
                .collect();
            log::trace!(
                "tuner: spectrum window bins={} max_hold_len={} sample_rate={} preview={:?}",
                spectrum.current_magnitudes.len(),
                spectrum.max_magnitudes.len(),
                spectrum.sample_rate,
                preview
            );
        }

        *self.last_spectrum.lock().unwrap() = Some(spectrum.clone());
        Some(spectrum)
    }

    fn shutdown(&self) {
        log::trace!("tuner: shutdown initiated");
        // Stop input stream
        if let Some(tx) = self.input_tx.lock().unwrap().take() {
            log::trace!("tuner: sending shutdown to input stream");
            let (ack_tx, ack_rx) = mpsc::channel();
            let _ = tx.send(stream::Control::Shutdown(ack_tx));
            let _ = ack_rx.recv_timeout(std::time::Duration::from_millis(500));
        } else {
            log::trace!("tuner: no input_tx present at shutdown");
        }

        if let Some(handle) = self.input_thread.lock().unwrap().take() {
            log::trace!("tuner: joining input thread");
            let _ = handle.join();
        } else {
            log::trace!("tuner: no input_thread to join");
        }

        // Clear analyzer & buffers
        self.analyzer.lock().unwrap().take();
        self.sample_buffer.lock().unwrap().clear();
        self.last_spectrum.lock().unwrap().take();
        self.max_hold.lock().unwrap().clear();
        self.activation_max_hold.lock().unwrap().clear();
        log::trace!("tuner: shutdown complete (state cleared)");
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
        {
            let mut cfg = self.config.lock().unwrap();
            *cfg = config.clone();
        }
        // Propagate to running analyzer without restart
        if let Some(analyzer) = self.analyzer.lock().unwrap().as_mut() {
            analyzer.set_config_for_tuner_stub(config.clone());
            if let Some(sr) = *self.sample_rate.lock().unwrap() {
                analyzer.set_sample_rate(sr as f64);
            }
        }
    }

    fn poll_spectrum(&self) -> Option<SpectrumData> {
        let res = self.process_window();
        if res.is_none() {
            log::trace!("tuner: poll_spectrum -> None");
        } else {
            log::trace!("tuner: poll_spectrum -> Some");
        }
        res
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
