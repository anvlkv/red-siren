use std::{
    collections::HashMap,
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

use common::{
    tuner::{Config, SensorData},
    NodeKey,
};
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::{audiounit::BigBlockAdapter, thingbuf::ThingBuf};
use parking_lot::RwLock;
use spectrum_analyzer::{
    samples_fft_to_spectrum, windows::hann_window, FrequencyLimit, FrequencySpectrum,
};

use super::adsr::Adsr;
use crate::{
    util::{hash_str, SComplex, S},
    ExcitementControl,
};

const ANALYZER_ID: u64 = hash_str(concat!(module_path!(), "::FFTAnalyzer"));
pub const FFT_WINDOW_SIZE: usize = 8192; // Power of 2 for FFT, good balance of frequency resolution vs latency

pub type SpectrumBuffer = Arc<ThingBuf<Arc<FrequencySpectrum>>>;

/// Custom AudioUnit that performs FFT analysis and excites sirens
#[derive(Clone)]
pub struct FFTAnalyzer {
    inner_net: BigBlockAdapter,
    window_thb: Arc<ThingBuf<f32>>,
    spectrum_thb: Arc<ThingBuf<Arc<FrequencySpectrum>>>,
    sensor_values: Arc<Vec<Shared>>,
    next_excitements_abs: Arc<RwLock<Option<HashMap<NodeKey, SComplex>>>>,

    // Configuration and controls
    sample_rate: f32,
    config: Config,
    excitement_controls: HashMap<NodeKey, ExcitementControl>,
    last_adsr: HashMap<NodeKey, (Adsr, SComplex)>,

    // processing thread
    processing_handle: Arc<JoinHandle<()>>,
    processing_running: Arc<AtomicBool>,
}

impl FFTAnalyzer {
    pub fn new(
        inner_net: Box<dyn AudioUnit>,
        config: Config,
        excitement_controls: HashMap<NodeKey, ExcitementControl>,
        spectrum_thb: Arc<ThingBuf<Arc<FrequencySpectrum>>>,
    ) -> Self {
        let window_thb = Arc::new(ThingBuf::new(FFT_WINDOW_SIZE * 2));
        let sensor_values = Arc::new(Vec::from_iter(config.sensor_data.iter().flat_map(|s| {
            [
                shared(s.min_frequency),
                shared(s.max_frequency),
                shared(s.min_magnitude),
                shared(s.max_magnitude),
            ]
        })));
        let last_adsr = HashMap::from_iter(
            excitement_controls
                .keys()
                .map(|k| (*k, (Adsr::new(DEFAULT_SR), SComplex::ZERO))),
        );
        let next_excitements_abs = Arc::new(RwLock::new(None));

        let sensor_data = config.sensor_data.clone();
        let (processing_handle, processing_running) = Self::init_handle(
            window_thb.clone(),
            next_excitements_abs.clone(),
            sensor_values.clone(),
            spectrum_thb.clone(),
            sensor_data,
            config.sample_rate.round() as u32,
        );

        let analyzer = Self {
            inner_net: BigBlockAdapter::new(inner_net),
            window_thb,
            sensor_values,
            last_adsr,
            next_excitements_abs,
            sample_rate: config.sample_rate,
            config,
            spectrum_thb,
            excitement_controls,
            processing_handle,
            processing_running,
        };

        // Validate initial setup
        analyzer.validate_sensor_controls();
        analyzer
    }

    /// Validate that sensor data NodeKeys match available excitement controls
    fn validate_sensor_controls(&self) {
        let missing_controls: Vec<NodeKey> = self
            .config
            .sensor_data
            .iter()
            .filter_map(|sensor| {
                if !self.excitement_controls.contains_key(&sensor.key) {
                    Some(sensor.key)
                } else {
                    None
                }
            })
            .collect();

        if !missing_controls.is_empty() {
            log::warn!(
                "FFT analyzer missing excitement controls for sensor keys: {:?}",
                missing_controls
            );
        }

        // Check for orphaned controls (controls without corresponding sensors)
        let sensor_keys: std::collections::HashSet<NodeKey> =
            self.config.sensor_data.iter().map(|s| s.key).collect();
        let orphaned_controls: Vec<NodeKey> = self
            .excitement_controls
            .keys()
            .filter(|k| !sensor_keys.contains(k))
            .copied()
            .collect();

        if !orphaned_controls.is_empty() {
            log::warn!(
                "FFT analyzer has orphaned excitement controls for keys: {:?}",
                orphaned_controls
            );
        }
    }

    #[allow(clippy::useless_conversion)]
    fn perform_fft_analysis(
        window: &[f32],
        sensor_inputs: &[f32],
        sample_rate: u32,
        sensor_data: &[SensorData],
        next_excitements_abs: &Arc<RwLock<Option<HashMap<NodeKey, SComplex>>>>,
        spectrum_thb: &Arc<ThingBuf<Arc<FrequencySpectrum>>>,
    ) {
        log::trace!(
            "fft_analyzer: perform_fft_analysis enter sample_rate={}",
            sample_rate
        );
        // Apply Hann window
        let windowed = hann_window(window);

        // Perform FFT using spectrum-analyzer
        let scaling = spectrum_analyzer::scaling::combined(&[
            &spectrum_analyzer::scaling::scale_20_times_log10,
            &spectrum_analyzer::scaling::scale_to_zero_to_one,
        ]);
        let spectrum = match samples_fft_to_spectrum(
            &windowed,
            sample_rate,
            FrequencyLimit::All,
            Some(&scaling),
        ) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                log::error!("FFT analysis failed: {}", e);
                log::trace!("fft_analyzer: FFT analysis failed early-exit");
                return;
            }
        };

        let mut next_excitements_lock = next_excitements_abs.write();
        let excitements_abs = next_excitements_lock.insert(HashMap::new());

        let spectrum_map = spectrum.to_map();

        // Process sensor excitements
        for (i, sensor) in sensor_data.iter().enumerate() {
            // Get sensor parameters from input channels
            let (min_freq, max_freq, min_mag, max_mag) = {
                let base_idx = i * 4;
                if base_idx + 3 < sensor_inputs.len() {
                    (
                        sensor_inputs[base_idx],
                        sensor_inputs[base_idx + 1],
                        sensor_inputs[base_idx + 2],
                        sensor_inputs[base_idx + 3],
                    )
                } else {
                    (
                        sensor.min_frequency,
                        sensor.max_frequency,
                        sensor.min_magnitude,
                        sensor.max_magnitude,
                    )
                }
            };

            let freq_range = min_freq.floor() as u32..max_freq.ceil() as u32;
            let mag_range = min_mag..max_mag;

            let spectrum_range = spectrum_map.range(freq_range.clone());

            if let Some((re, im)) = spectrum_range
                .filter(|(_, mag)| mag_range.contains(mag))
                .max_by(|(_, a_mag), (_, b_mag)| {
                    a_mag
                        .partial_cmp(b_mag)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .and_then(|(&freq, &mag)| {
                    freq_range.clone().position(|f| f == freq).map(|p| {
                        (
                            (mag - min_mag) as S / (max_mag - min_mag) as S,
                            p as S / freq_range.len() as S,
                        )
                    })
                })
            {
                excitements_abs.insert(sensor.key, SComplex::new(re, im));
            }
        }

        if let Err(full) = spectrum_thb.push(Arc::new(spectrum)) {
            log::trace!("fft_analyzer: failed to push spectrum data, catching up");
            _ = spectrum_thb.pop();
            match spectrum_thb.push(full.into_inner()) {
                Ok(_) => {}
                Err(_) => {
                    log::error!("fft_analyzer: failed to push spectrum data after pop");
                }
            }
        }
    }

    fn init_handle(
        window_thb: Arc<ThingBuf<f32>>,
        next_excitements_abs: Arc<RwLock<Option<HashMap<NodeKey, SComplex>>>>,
        sensor_values: Arc<Vec<Shared>>,
        spectrum_thb: SpectrumBuffer,
        sensor_data: Vec<SensorData>,
        sample_rate: u32,
    ) -> (Arc<JoinHandle<()>>, Arc<AtomicBool>) {
        let running = Arc::new(AtomicBool::new(true));
        let running_thread = running.clone();
        let join = spawn(move || {
            log::info!("FFT analyzer thread started");
            loop {
                if !running_thread.load(Ordering::SeqCst) {
                    log::info!("FFT analyzer thread stopped");
                    break;
                }
                if FFT_WINDOW_SIZE
                    .checked_sub(window_thb.len())
                    .is_some_and(|s| s != 0)
                {
                    sleep(Duration::from_micros(500));
                } else {
                    let window: [f32; FFT_WINDOW_SIZE] =
                        core::array::from_fn(|_| window_thb.pop().unwrap_or_default());
                    let sensor_inputs = sensor_values.iter().map(|s| s.value()).collect::<Vec<_>>();

                    Self::perform_fft_analysis(
                        &window,
                        &sensor_inputs,
                        sample_rate,
                        &sensor_data,
                        &next_excitements_abs,
                        &spectrum_thb,
                    );
                }
            }
        });

        (Arc::new(join), running)
    }

    fn restart_processing(&mut self) {
        self.processing_running.store(false, Ordering::SeqCst);
        let sensor_data = self.config.sensor_data.clone();
        let (processing_handle, processing_running) = Self::init_handle(
            self.window_thb.clone(),
            self.next_excitements_abs.clone(),
            self.sensor_values.clone(),
            self.spectrum_thb.clone(),
            sensor_data,
            self.config.sample_rate.round() as u32,
        );

        let old_handle = mem::replace(&mut self.processing_handle, processing_handle);
        if let Some(handle) = Arc::into_inner(old_handle) {
            handle.join().unwrap();
            log::debug!("FFT analyzer thread stop completed (join)");
        }

        self.processing_running = processing_running;
    }

    #[allow(clippy::unnecessary_cast)]
    fn tick_adsr(&mut self) {
        self.excitement_controls.iter().for_each(|(key, control)| {
            if let Some((adsr, _)) = self.last_adsr.get_mut(key) {
                let adsr_value = adsr.tick();
                control.set_value(adsr_value);
            }
        });

        if let Some(excitements_abs) = self
            .next_excitements_abs
            .try_write()
            .and_then(|mut next| next.take())
        {
            for (key, excitement) in excitements_abs.into_iter() {
                if let Some((adsr, prev_excitement)) = self.last_adsr.get_mut(&key) {
                    adsr.update(*prev_excitement, excitement);
                    *prev_excitement = excitement;
                }
            }
        }
    }
}

impl AudioUnit for FFTAnalyzer {
    fn inputs(&self) -> usize {
        self.inner_net.inputs() + self.config.sensor_data.len() * 4
    }

    fn outputs(&self) -> usize {
        self.inner_net.outputs()
    }

    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        // Process through inner network with all inputs
        self.inner_net.tick(&input[0..1], output);

        // Extract sensor inputs from additional channels
        let sensor_inputs = &input[1..];

        for (s_val, ctrl) in sensor_inputs.iter().zip(self.sensor_values.iter()) {
            ctrl.set_value(*s_val);
        }

        if sensor_inputs.len() != self.sensor_values.len() {
            log::warn!("Mismatched number of sensor inputs and controls");
        }

        if self.window_thb.push(output[0]).is_err() {
            log::trace!("failed to push one sample")
        }

        self.tick_adsr();
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Extract sensor inputs from additional channels
        let (audio_input, sensor_inputs): (BufferRef, Vec<f32>) = {
            let buffer_ref = BufferRef::new(input.channel(0));

            let mut values = Vec::new();
            for ch in 1..input.channels() {
                if size > 0 {
                    // Take the first sample from each sensor channel
                    let simd_val = input.at(ch, 0);
                    let array = simd_val.to_array();
                    values.push(array[0]);
                }
            }
            (buffer_ref, values)
        };

        // Process through inner network with all inputs
        self.inner_net.process(size, &audio_input, output);

        self.sensor_values
            .iter()
            .zip(sensor_inputs)
            .for_each(|(ctrl, input)| {
                ctrl.set_value(input);
            });

        'outer: for chunk in output.channel(0) {
            for s in chunk.as_array_ref() {
                if self.window_thb.push(*s).is_err() {
                    break 'outer;
                }
            }
        }

        self.tick_adsr();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner_net.set_sample_rate(sample_rate);
        self.sample_rate = sample_rate as f32;
        self.last_adsr.iter_mut().for_each(|(_, (adsr, _))| {
            adsr.update_sample_rate(sample_rate);
        });
        self.restart_processing();
    }

    fn reset(&mut self) {
        self.inner_net.reset();
        while self.window_thb.pop().is_some() {}

        // Reset all siren controls
        for siren_control in self.excitement_controls.values() {
            siren_control.set_value((0.0, 0.0));
        }

        self.restart_processing();

        log::trace!("fft_analyzer: reset complete",);
    }

    fn allocate(&mut self) {
        self.inner_net.allocate();
    }

    fn route(&mut self, input: &SignalFrame, frequency: f64) -> SignalFrame {
        self.inner_net.route(input, frequency)
    }

    fn get_id(&self) -> u64 {
        ANALYZER_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>() + self.inner_net.footprint()
    }
}

#[cfg(test)]
mod tests {
    use common::tuner::SensorData;

    use super::*;

    #[test]
    fn test_fft_analyzer_creation() {
        // Create a simple passthrough network
        let inner_net = Box::new(pass());

        // Create test config
        let config = Config {
            sample_rate: 44100.0,
            sensor_data: vec![SensorData {
                key: NodeKey::new(0, 0),
                min_frequency: 100.0,
                max_frequency: 500.0,
                min_magnitude: 0.01,
                max_magnitude: 1.0,
            }],
        };

        // Create siren controls
        let mut siren_controls = HashMap::new();
        let control = ExcitementControl::default();
        siren_controls.insert(NodeKey::new(0, 0), control.clone());

        let spectrum_thb = Arc::new(ThingBuf::new(10));

        // Create analyzer
        let analyzer = FFTAnalyzer::new(inner_net, config, siren_controls, spectrum_thb);

        // Test basic properties
        assert_eq!(analyzer.inputs(), 5);
        assert_eq!(analyzer.outputs(), 1);

        // Test initial values
        assert_eq!(control.value().re, 0.0);
    }

    #[test]
    fn test_sensor_excitement_threshold() {
        let inner_net = Box::new(pass());

        // Create test config
        let config = Config {
            sample_rate: 44100.0,
            sensor_data: vec![SensorData {
                key: NodeKey::new(0, 0),
                min_frequency: 440.0,
                max_frequency: 460.0,
                min_magnitude: 0.05,
                max_magnitude: 0.5,
            }],
        };

        let mut siren_controls = HashMap::new();
        let control = ExcitementControl::default();
        siren_controls.insert(NodeKey::new(0, 0), control.clone());

        let spectrum_thb = Arc::new(ThingBuf::new(10));

        let mut analyzer = FFTAnalyzer::new(inner_net, config, siren_controls, spectrum_thb);

        // Reset should clear everything
        analyzer.reset();
        assert_eq!(control.value().re, 0.0);
    }
}
