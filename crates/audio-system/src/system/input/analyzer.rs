use std::collections::HashMap;

use common::{tuner::Config, NodeKey};
use fundsp::audiounit::BigBlockAdapter;
use fundsp::hacker32::prelude::*;
use spectrum_analyzer::{
    samples_fft_to_spectrum, windows::hann_window, FrequencyLimit, FrequencySpectrum,
};

use crate::util::hash_str;
use ringbuf::{
    traits::{Consumer, Producer, SplitRef},
    StaticRb,
};

const ANALYZER_ID: u64 = hash_str(concat!(module_path!(), "::FFTAnalyzer"));

pub const FFT_WINDOW_SIZE: usize = 2048; // Power of 2 for FFT, good balance of frequency resolution vs latency

/// Custom AudioUnit that performs FFT analysis and activates sirens
pub struct FFTAnalyzer {
    inner_net: BigBlockAdapter,
    window_rb: StaticRb<f32, FFT_WINDOW_SIZE>,
    window_size: usize,
    sample_count: usize,

    // Configuration and controls
    sample_rate: f32,
    config: Config,
    activation_controls: HashMap<NodeKey, Shared>,

    // Spectrum data storage
    spectrum: Option<FrequencySpectrum>,
}

impl FFTAnalyzer {
    pub fn new(
        inner_net: Box<dyn AudioUnit>,
        window_size: usize,
        config: Config,
        activation_controls: HashMap<NodeKey, Shared>,
    ) -> Self {
        Self {
            inner_net: BigBlockAdapter::new(inner_net),
            window_rb: StaticRb::<f32, FFT_WINDOW_SIZE>::default(),
            window_size,
            sample_count: 0,
            // peak_frequency: peak_frequency.clone(),
            // spectral_centroid: spectral_centroid.clone(),
            // rms_level: rms_level.clone(),
            sample_rate: config.sample_rate,
            config,
            activation_controls,
            spectrum: None,
        }
    }

    pub fn new_tuner_stub(
        inner_net: Box<dyn AudioUnit>,
        window_size: usize,
        config: Config,
    ) -> Self {
        let activation_controls = Self::activation_controls_from_config(&config);
        Self::new(inner_net, window_size, config, activation_controls)
    }

    fn activation_controls_from_config(config: &Config) -> HashMap<NodeKey, Shared> {
        HashMap::from_iter(config.sensor_data.iter().map(|d| (d.key, shared(0.0))))
    }

    /// Update analyzer configuration at runtime (sensor thresholds, sample rate, keys)
    pub fn set_config_for_tuner_stub(&mut self, config: Config) {
        let activation_controls = Self::activation_controls_from_config(&config);
        self.sample_rate = config.sample_rate;
        self.config = config;
        self.activation_controls = activation_controls;
    }

    /// Get the latest spectrum data (frequencies and magnitudes)
    pub fn get_spectrum_data(&self) -> Option<(Vec<f32>, Vec<f32>)> {
        self.spectrum.as_ref().map(|spectrum| {
            let data = spectrum.data();
            let mut frequencies = Vec::with_capacity(data.len());
            let mut magnitudes = Vec::with_capacity(data.len());
            for (freq, mag) in data {
                frequencies.push(freq.val());
                magnitudes.push(mag.val());
            }
            (frequencies, magnitudes)
        })
    }

    /// Get the current frequency spectrum
    pub fn get_spectrum(&self) -> Option<&FrequencySpectrum> {
        self.spectrum.as_ref()
    }

    /// Get current sensor activation levels
    pub fn get_sensor_activations(&self) -> HashMap<NodeKey, f32> {
        HashMap::from_iter(
            self.activation_controls
                .iter()
                .map(|(k, v)| (*k, v.value())),
        )
    }

    /// Analyze a full window of raw input samples by running them through the preamp,
    /// collecting the processed mono output, and performing FFT on that processed window.
    pub fn analyze_window(&mut self, samples: &[f32]) {
        if samples.len() != self.window_size {
            log::trace!(
                "fft_analyzer: analyze_window wrong size (got {}, expected {})",
                samples.len(),
                self.window_size
            );
            return;
        }

        self.inner_net.set_sample_rate(self.sample_rate as f64);

        // Prepare input/output slices for process_big (mono)
        let input_slices: [&[f32]; 1] = [samples];
        let mut processed = vec![0.0f32; self.window_size];
        let mut output_slices: [&mut [f32]; 1] = [processed.as_mut_slice()];

        self.inner_net
            .process_big(self.window_size, &input_slices, &mut output_slices);

        self.perform_fft_analysis(&processed);
    }

    fn perform_fft_analysis(&mut self, window: &[f32]) {
        log::trace!(
            "fft_analyzer: perform_fft_analysis enter window_size={} sample_rate={}",
            self.window_size,
            self.sample_rate
        );
        // Apply Hann window
        let windowed = hann_window(window);

        // Perform FFT using spectrum-analyzer
        let scaling = spectrum_analyzer::scaling::combined(&[
            &spectrum_analyzer::scaling::divide_by_N_sqrt,
            &spectrum_analyzer::scaling::scale_20_times_log10,
        ]);
        let spectrum = match samples_fft_to_spectrum(
            &windowed,
            self.sample_rate as u32,
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

        // Store spectrum for external access
        self.spectrum = Some(spectrum);
        let spectrum = self.spectrum.as_ref().unwrap();

        // Process sensor activations
        for sensor in &self.config.sensor_data {
            // Use FrequencySpectrum: closest bin at sensor center frequency
            let center = 0.5 * (sensor.min_frequency + sensor.max_frequency);
            let (f, v) = spectrum.freq_val_closest(center);
            let freq = f.val();
            let peak_db = v.val();

            // Activate only if the closest bin actually lies inside the sensor band
            let in_band = freq >= sensor.min_frequency && freq <= sensor.max_frequency;

            // Distance-based activation: interpolate between min/max magnitude
            let activation = if !in_band {
                0.0
            } else if peak_db <= sensor.min_magnitude {
                f32::EPSILON
            } else {
                // Linear interpolation/extrapolation based on min/max range
                (peak_db - sensor.min_magnitude) / (sensor.max_magnitude - sensor.min_magnitude)
            };

            if log::log_enabled!(log::Level::Trace) && activation > 0.01 {
                log::trace!(
                    "fft_analyzer: sensor key={:?} range=[{:.1},{:.1}] activation={:.3}",
                    sensor.key,
                    sensor.min_frequency,
                    sensor.max_frequency,
                    activation
                );
            }

            // Update siren control if we have a control for this sensor
            if let Some(siren_control) = self.activation_controls.get(&sensor.key) {
                siren_control.set_value(activation);
            } else {
                log::warn!(
                    "fft_analyzer: no siren control found for sensor key={:?}",
                    sensor.key
                );
            }
        }
    }
}

impl Clone for FFTAnalyzer {
    fn clone(&self) -> Self {
        Self {
            inner_net: self.inner_net.clone(),
            window_rb: StaticRb::<f32, FFT_WINDOW_SIZE>::default(),
            window_size: self.window_size,
            sample_count: 0,
            sample_rate: self.sample_rate,
            config: self.config.clone(),
            activation_controls: self.activation_controls.clone(),
            spectrum: None,
        }
    }
}

impl AudioUnit for FFTAnalyzer {
    fn inputs(&self) -> usize {
        self.inner_net.inputs()
    }

    fn outputs(&self) -> usize {
        self.inner_net.outputs()
    }

    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        // Process through inner network first
        self.inner_net.tick(input, output);

        // Collect samples for FFT analysis (from first output channel if available)
        if !output.is_empty() {
            let sample = output[0];
            {
                let mut analyze_window_opt: Option<Vec<f32>> = None;
                {
                    let rb = &mut self.window_rb;
                    let (mut prod, mut cons) = rb.split_ref();
                    // Try to push; if ring is full, drain one and retry once.
                    let mut pushed_ok = prod.try_push(sample).is_ok();
                    if !pushed_ok {
                        let _: Option<f32> = cons.try_pop();
                        if self.sample_count > 0 {
                            self.sample_count -= 1;
                        }
                        pushed_ok = prod.try_push(sample).is_ok();
                    }
                    if pushed_ok {
                        self.sample_count += 1;

                        // Analyze exactly when we have a full window accumulated.
                        if self.sample_count >= self.window_size {
                            log::trace!(
                                "fft_analyzer: tick window complete samples={}",
                                self.sample_count
                            );
                            // Drain window samples in order
                            let mut window = Vec::with_capacity(self.window_size);
                            while window.len() < self.window_size {
                                let v: Option<f32> = cons.try_pop();
                                if let Some(s) = v {
                                    window.push(s);
                                } else {
                                    break;
                                }
                            }
                            if window.len() == self.window_size {
                                // Reset fill count after analysis
                                self.sample_count = 0;
                                analyze_window_opt = Some(window);
                            } else {
                                log::trace!(
                                    "fft_analyzer: insufficient samples to analyze (have {}, need {})",
                                    window.len(),
                                    self.window_size
                                );
                            }
                        }
                    }
                }
                if let Some(window) = analyze_window_opt {
                    self.perform_fft_analysis(&window);
                }
            }
        }
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // First, let the inner network (preamp) process audio
        self.inner_net.process(size, input, output);

        // Collect samples for FFT analysis (from first output channel)
        if output.channels() > 0 && size > 0 {
            for i in 0..size {
                let simd_val = output.at(0, i);
                let array = simd_val.to_array();
                let sample = array[0];
                {
                    let mut analyze_window_opt: Option<Vec<f32>> = None;
                    {
                        let rb = &mut self.window_rb;
                        let (mut prod, mut cons) = rb.split_ref();
                        // Try to push; on full buffer drain one and retry once.
                        let mut pushed_ok = prod.try_push(sample).is_ok();
                        if !pushed_ok {
                            let _: Option<f32> = cons.try_pop();
                            if self.sample_count > 0 {
                                self.sample_count -= 1;
                            }
                            pushed_ok = prod.try_push(sample).is_ok();
                        }
                        if pushed_ok {
                            self.sample_count += 1;

                            // Analyze exactly when we have a full window accumulated.
                            if self.sample_count >= self.window_size {
                                log::trace!(
                                    "fft_analyzer: process window complete samples={}",
                                    self.sample_count
                                );
                                let mut window = Vec::with_capacity(self.window_size);
                                while window.len() < self.window_size {
                                    let v: Option<f32> = cons.try_pop();
                                    if let Some(s) = v {
                                        window.push(s);
                                    } else {
                                        break;
                                    }
                                }
                                if window.len() == self.window_size {
                                    // Reset fill count after analysis
                                    self.sample_count = 0;
                                    analyze_window_opt = Some(window);
                                } else {
                                    log::trace!(
                                        "fft_analyzer: insufficient samples to analyze (have {}, need {})",
                                        window.len(),
                                        self.window_size
                                    );
                                }
                            }
                        }
                    }
                    if let Some(window) = analyze_window_opt {
                        self.perform_fft_analysis(&window);
                    }
                }
            }
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner_net.set_sample_rate(sample_rate);
        self.sample_rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.inner_net.reset();
        self.sample_count = 0;
        {
            let rb = &mut self.window_rb;
            let (_, mut cons) = rb.split_ref();
            loop {
                let v: Option<f32> = cons.try_pop();
                if v.is_none() {
                    break;
                }
            }
        }

        // Reset all siren controls
        for siren_control in self.activation_controls.values() {
            siren_control.set_value(0.0);
        }

        // Clear spectrum data
        self.spectrum = None;

        log::trace!(
            "fft_analyzer: reset complete window_size={} sample_rate={}",
            self.window_size,
            self.sample_rate
        );
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
            fft_size: 2048,
            sensor_data: vec![SensorData {
                key: NodeKey(0, 0),
                min_frequency: 100.0,
                max_frequency: 500.0,
                min_magnitude: 0.01,
                max_magnitude: 1.0,
            }],
        };

        // Create siren controls
        let mut siren_controls = HashMap::new();
        let control = shared(0.0);
        siren_controls.insert(NodeKey(0, 0), control.clone());

        // Create analyzer
        let analyzer = FFTAnalyzer::new(inner_net, FFT_WINDOW_SIZE, config, siren_controls);

        // Test basic properties
        assert_eq!(analyzer.inputs(), 1);
        assert_eq!(analyzer.outputs(), 1);
        assert_eq!(analyzer.window_size, FFT_WINDOW_SIZE);

        // Test initial values
        assert_eq!(control.value(), 0.0);
    }

    #[test]
    fn test_sensor_activation_threshold() {
        let inner_net = Box::new(pass());

        // Create test config
        let config = Config {
            sample_rate: 44100.0,
            fft_size: 2048,
            sensor_data: vec![SensorData {
                key: NodeKey(0, 0),
                min_frequency: 440.0,
                max_frequency: 460.0,
                min_magnitude: 0.05,
                max_magnitude: 0.5,
            }],
        };

        let mut siren_controls = HashMap::new();
        let control = shared(0.0);
        siren_controls.insert(NodeKey(0, 0), control.clone());

        let mut analyzer = FFTAnalyzer::new(inner_net, FFT_WINDOW_SIZE, config, siren_controls);

        // Reset should clear everything
        analyzer.reset();
        assert_eq!(control.value(), 0.0);
        assert_eq!(analyzer.sample_count, 0);
    }
}
