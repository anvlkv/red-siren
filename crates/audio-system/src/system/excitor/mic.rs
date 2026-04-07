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
use fundsp::prelude::*;
use fundsp::{audiounit::BigBlockAdapter, thingbuf::ThingBuf};
use num_complex::Complex;
use parking_lot::RwLock;
use spectrum_analyzer::{
    samples_fft_to_spectrum, windows::hann_window, FrequencyLimit, FrequencySpectrum,
};

use super::adsr::Adsr;
use crate::{util::hash_str, ExcitementControl};

const ANALYZER_ID: u64 = hash_str(concat!(module_path!(), "::FFTAnalyzer"));
pub const FFT_WINDOW_SIZE: usize = 8192; // Power of 2 for FFT, good balance of frequency resolution vs latency

pub type SpectrumBuffer = Arc<ThingBuf<Arc<FrequencySpectrum>>>;

/// Custom AudioUnit that performs FFT analysis and excites sirens
#[derive(Clone)]
pub struct FFTAnalyzer<S: Real + Float + 'static> {
    inner_net: BigBlockAdapter,
    window_thb: Arc<ThingBuf<f32>>,
    spectrum_thb: SpectrumBuffer,
    sensor_values: Arc<ThingBuf<Vec<SensorData>>>,
    frequency_limit: Arc<ThingBuf<Option<FrequencyLimit>>>,
    next_excitements_abs: Arc<RwLock<HashMap<NodeKey, Complex<S>>>>,
    sensors_slice: Vec<f32>,

    // Configuration and controls
    sample_rate: f32,
    config: Config,
    excitement_controls: HashMap<NodeKey, ExcitementControl>,
    adsr_s: HashMap<NodeKey, (Adsr<S>, Complex<S>)>,

    // processing thread
    processing_handle: Arc<JoinHandle<()>>,
    processing_running: Arc<AtomicBool>,
}

impl<S: Real + Float + 'static> FFTAnalyzer<S> {
    pub fn new(
        inner_net: Box<dyn AudioUnit>,
        config: Config,
        excitement_controls: HashMap<NodeKey, ExcitementControl>,
        spectrum_thb: SpectrumBuffer,
    ) -> Self {
        let window_thb = Arc::new(ThingBuf::new(FFT_WINDOW_SIZE * 4));
        let sensor_values = Arc::new(ThingBuf::new(2));
        sensor_values.push(config.sensor_data.clone()).unwrap();
        let frequency_limit = Arc::new(ThingBuf::new(2));
        frequency_limit
            .push(Some(match config.frequency_range {
                (None, None) => FrequencyLimit::All,
                (None, Some(max)) => FrequencyLimit::Max(max),
                (Some(min), None) => FrequencyLimit::Min(min),
                (Some(min), Some(max)) => FrequencyLimit::Range(min, max),
            }))
            .unwrap();
        let last_adsr = HashMap::from_iter(excitement_controls.keys().map(|k| {
            (
                *k,
                (
                    Adsr::new(DEFAULT_SR),
                    Complex::<S>::new(S::zero(), S::zero()),
                ),
            )
        }));
        let next_excitements_abs = Arc::new(RwLock::new(HashMap::with_capacity(
            config.sensor_data.len(),
        )));

        let (processing_handle, processing_running) = Self::init_handle(
            window_thb.clone(),
            next_excitements_abs.clone(),
            sensor_values.clone(),
            frequency_limit.clone(),
            spectrum_thb.clone(),
            config.sample_rate.round() as u32,
        );

        let analyzer = Self {
            sensors_slice: vec![0_f32; config.sensor_data.len() * 4],
            inner_net: BigBlockAdapter::new(inner_net),
            window_thb,
            sensor_values,
            frequency_limit,
            adsr_s: last_adsr,
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

    fn extract_audio_and_update_from_inputs(&self, input: &[f32]) -> f32 {
        let num_sensor_inputs = self.num_sensor_input();

        // [0] = audio, [1..1+4N) = sensors, [1+4N .. 1+4N+2) = [min, max]
        let sensors = &input[1..1 + num_sensor_inputs];
        let limits = &input[input.len() - 2..];
        self.update_from_inputs(sensors, limits);

        input[0]
    }

    fn update_from_inputs(&self, sensors_input: &[f32], limits_input: &[f32]) {
        if let Some(err) = self
            .sensor_values
            .push(
                self.config
                    .sensor_data
                    .iter()
                    .copied()
                    .zip(sensors_input.chunks_exact(4))
                    .map(|(mut data, values)| {
                        data.min_frequency = values[0];
                        data.max_frequency = values[1];
                        data.min_magnitude = values[2];
                        data.max_magnitude = values[3];
                        data
                    })
                    .collect(),
            )
            .err()
        {
            _ = self.sensor_values.pop();
            _ = self.sensor_values.push(err.into_inner());
        }
        let min_freq_value = limits_input[0].min(limits_input[1]);
        let max_freq_value = limits_input[1].max(limits_input[0]);

        if let Some(err) = self
            .frequency_limit
            .push(Some(
                match (min_freq_value.is_finite(), max_freq_value.is_finite()) {
                    (true, true) => FrequencyLimit::Range(min_freq_value, max_freq_value),
                    (true, false) => FrequencyLimit::Min(min_freq_value),
                    (false, true) => FrequencyLimit::Max(max_freq_value),
                    (false, false) => FrequencyLimit::All,
                },
            ))
            .err()
        {
            _ = self.frequency_limit.pop();
            _ = self.frequency_limit.push(err.into_inner());
        }
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
        sensor_data: &[SensorData],
        &freq_limit: &FrequencyLimit,
        sample_rate: u32,
        next_excitements_abs: &Arc<RwLock<HashMap<NodeKey, Complex<S>>>>,
        spectrum_thb: &SpectrumBuffer,
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
        let spectrum =
            match samples_fft_to_spectrum(&windowed, sample_rate, freq_limit, Some(&scaling)) {
                Ok(spectrum) => Arc::new(spectrum),
                Err(e) => {
                    log::error!("FFT analysis failed: {}", e);
                    log::trace!("fft_analyzer: FFT analysis failed early-exit");
                    return;
                }
            };

        {
            let spectrum_map = spectrum.to_map();
            let mut next_excitements = next_excitements_abs.write();

            // Process sensor excitements
            for sensor in sensor_data.iter() {
                let SensorData {
                    key,
                    min_frequency,
                    max_frequency,
                    min_magnitude,
                    max_magnitude,
                } = *sensor;

                let freq_range = min_frequency.floor() as u32..max_frequency.ceil() as u32;
                let mag_range = min_magnitude..max_magnitude;

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
                                S::from_f32(
                                    (mag - min_magnitude) / (max_magnitude - min_magnitude),
                                ),
                                S::from_f32(p as f32 / freq_range.len() as f32),
                            )
                        })
                    })
                {
                    _ = next_excitements.insert(key, Complex::<S>::new(re, im));
                } else {
                    _ = next_excitements.remove(&key);
                }
            }
        }

        if let Err(full) = spectrum_thb.push(spectrum) {
            log::trace!("fft_analyzer: failed to push spectrum data, catching up");
            _ = spectrum_thb.pop();
            _ = spectrum_thb.push(full.into_inner());
        }
    }

    fn init_handle(
        window_thb: Arc<ThingBuf<f32>>,
        next_excitements_abs: Arc<RwLock<HashMap<NodeKey, Complex<S>>>>,
        sensor_values: Arc<ThingBuf<Vec<SensorData>>>,
        freq_limit: Arc<ThingBuf<Option<FrequencyLimit>>>,
        spectrum_thb: SpectrumBuffer,
        sample_rate: u32,
    ) -> (Arc<JoinHandle<()>>, Arc<AtomicBool>) {
        let running = Arc::new(AtomicBool::new(true));
        let running_thread = running.clone();
        let join = spawn(move || {
            log::info!("FFT analyzer thread started");
            let mut last_sensor_data: Vec<SensorData> = sensor_values.pop().unwrap_or_default();
            let mut last_freq_limit: FrequencyLimit =
                freq_limit.pop().flatten().unwrap_or(FrequencyLimit::All);

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
                    let sensor_values_data = sensor_values.pop();
                    let sensor_data = sensor_values_data.as_ref().unwrap_or(&last_sensor_data);

                    let freq_limit = freq_limit.pop().flatten().unwrap_or(last_freq_limit);

                    let window: [f32; FFT_WINDOW_SIZE] =
                        core::array::from_fn(|_| window_thb.pop().unwrap_or_default());

                    Self::perform_fft_analysis(
                        &window,
                        sensor_data,
                        &freq_limit,
                        sample_rate,
                        &next_excitements_abs,
                        &spectrum_thb,
                    );

                    if let Some(data) = sensor_values_data {
                        last_sensor_data = data;
                    }

                    last_freq_limit = freq_limit;
                }
            }
        });

        (Arc::new(join), running)
    }

    fn restart_processing(&mut self) {
        self.processing_running.store(false, Ordering::SeqCst);

        if let Err(e) = self.sensor_values.push(self.config.sensor_data.clone()) {
            self.sensor_values.pop().unwrap();
            self.sensor_values.push(e.into_inner()).unwrap();
        }
        if let Err(e) = self
            .frequency_limit
            .push(Some(match self.config.frequency_range {
                (None, None) => FrequencyLimit::All,
                (None, Some(max)) => FrequencyLimit::Max(max),
                (Some(min), None) => FrequencyLimit::Min(min),
                (Some(min), Some(max)) => FrequencyLimit::Range(min, max),
            }))
        {
            self.frequency_limit.pop().unwrap();
            self.frequency_limit.push(e.into_inner()).unwrap();
        }

        let (processing_handle, processing_running) = Self::init_handle(
            self.window_thb.clone(),
            self.next_excitements_abs.clone(),
            self.sensor_values.clone(),
            self.frequency_limit.clone(),
            self.spectrum_thb.clone(),
            self.config.sample_rate.round() as u32,
        );

        let old_handle = mem::replace(&mut self.processing_handle, processing_handle);
        if let Some(handle) = Arc::into_inner(old_handle) {
            handle.join().unwrap();
            log::debug!("FFT analyzer thread stop completed (join)");
        }

        self.processing_running = processing_running;
    }

    fn tick_adsr(&mut self) {
        self.excitement_controls.iter().for_each(|(key, control)| {
            if let Some((adsr, _)) = self.adsr_s.get_mut(key) {
                let adsr_value = adsr.tick();
                control.set_value(adsr_value);
            }
        });

        {
            if let Some(mut excitements_abs) = self
                .next_excitements_abs
                .try_write()
                .filter(|m| !m.is_empty())
            {
                for (key, excitement) in excitements_abs.drain() {
                    if let Some((adsr, prev_excitement)) = self.adsr_s.get_mut(&key) {
                        adsr.update(*prev_excitement, excitement);
                        *prev_excitement = excitement;
                    }
                }
            }
        }
    }

    fn num_sensor_input(&self) -> usize {
        self.config.sensor_data.len() * 4
    }
}

impl<S: Real + Float + 'static> AudioUnit for FFTAnalyzer<S> {
    fn inputs(&self) -> usize {
        self.inner_net.inputs() + self.num_sensor_input() + 2
    }

    fn outputs(&self) -> usize {
        self.inner_net.outputs()
    }

    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        let audio = self.extract_audio_and_update_from_inputs(input);
        // Process through inner network with all inputs
        self.inner_net.tick(&[audio], output);

        _ = self.window_thb.push(output[0]);

        self.tick_adsr();
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        let num_sensor_inputs = self.num_sensor_input();

        // Channel layout: [ audio | sensors... | min_freq | max_freq ]
        let audio_in = input.subset(0, 1);
        let sensors_in = input.subset(1, num_sensor_inputs);
        let limits_in = input.subset(input.channels() - 2, 2);

        self.inner_net.process(size, &audio_in, output);
        let processed = output.buffer_ref();

        let blocks = size / SIMD_LEN;
        let rem = size % SIMD_LEN;

        // Process full SIMD blocks
        for i in 0..blocks {
            let audio_vec = processed.at(0, i);
            let lim0_vec = limits_in.at(0, i);
            let lim1_vec = limits_in.at(1, i);
            let audio_vec = audio_vec.as_array();
            let lim0_vec = lim0_vec.as_array();
            let lim1_vec = lim1_vec.as_array();

            for lane in 0..SIMD_LEN {
                // Gather per-sample sensor values across channels
                self.sensors_slice
                    .iter_mut()
                    .enumerate()
                    .for_each(|(ch, sample)| {
                        *sample = sensors_in.at(ch, i).as_array()[lane];
                    });

                let limits_pair = [lim0_vec[lane], lim1_vec[lane]];
                self.update_from_inputs(&self.sensors_slice, &limits_pair);

                let _ = self.window_thb.push(audio_vec[lane]);
            }

            self.tick_adsr();
        }

        // Handle tail (if any) using the last available block vectors
        if rem > 0 {
            // The last block index usable for .at() is blocks - 1 if blocks > 0.
            // If there were no full blocks, use index 0; Fundsp provides a valid vector view.
            let idx = if blocks > 0 { blocks - 1 } else { 0 };

            let audio_vec = processed.at(0, idx);
            let lim0_vec = limits_in.at(0, idx);
            let lim1_vec = limits_in.at(1, idx);
            let audio_vec = audio_vec.as_array();
            let lim0_vec = lim0_vec.as_array();
            let lim1_vec = lim1_vec.as_array();

            for lane in 0..rem {
                self.sensors_slice
                    .iter_mut()
                    .enumerate()
                    .for_each(|(ch, sample)| {
                        *sample = sensors_in.at(ch, idx).as_array()[lane];
                    });

                let limits_pair = [lim0_vec[lane], lim1_vec[lane]];
                self.update_from_inputs(&self.sensors_slice, &limits_pair);

                let _ = self.window_thb.push(audio_vec[lane]);
            }

            self.tick_adsr();
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner_net.set_sample_rate(sample_rate);
        self.sample_rate = sample_rate as f32;
        self.adsr_s.iter_mut().for_each(|(_, (adsr, _))| {
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
            fft_size: FFT_WINDOW_SIZE,
            sensor_data: vec![SensorData {
                key: NodeKey::new(0, 0),
                min_frequency: 100.0,
                max_frequency: 500.0,
                min_magnitude: 0.01,
                max_magnitude: 1.0,
            }],
            ..Default::default()
        };

        // Create siren controls
        let mut siren_controls = HashMap::new();
        let control = ExcitementControl::default();
        siren_controls.insert(NodeKey::new(0, 0), control.clone());

        let spectrum_thb = Arc::new(ThingBuf::new(10));

        // Create analyzer
        let analyzer = FFTAnalyzer::<f32>::new(inner_net, config, siren_controls, spectrum_thb);

        // Test basic properties
        assert_eq!(analyzer.inputs(), 7);
        assert_eq!(analyzer.outputs(), 1);

        // Test initial values
        assert_eq!(control.value::<f32>().re, 0.0);
    }

    #[test]
    fn test_sensor_excitement_threshold() {
        let inner_net = Box::new(pass());

        // Create test config
        let config = Config {
            sample_rate: 44100.0,
            fft_size: FFT_WINDOW_SIZE,
            sensor_data: vec![SensorData {
                key: NodeKey::new(0, 0),
                min_frequency: 440.0,
                max_frequency: 460.0,
                min_magnitude: 0.05,
                max_magnitude: 0.5,
            }],
            ..Default::default()
        };

        let mut siren_controls = HashMap::new();
        let control = ExcitementControl::default();
        siren_controls.insert(NodeKey::new(0, 0), control.clone());

        let spectrum_thb = Arc::new(ThingBuf::new(10));

        let mut analyzer = FFTAnalyzer::<f32>::new(inner_net, config, siren_controls, spectrum_thb);

        // Reset should clear everything
        analyzer.reset();
        assert_eq!(control.value::<f32>().re, 0.0);
    }
}
