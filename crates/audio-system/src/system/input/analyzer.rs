use std::{collections::HashMap, sync::Arc};

use common::{tuner::Config, NodeKey};
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::{audiounit::BigBlockAdapter, thingbuf::ThingBuf};
use spectrum_analyzer::{
    samples_fft_to_spectrum, windows::hann_window, FrequencyLimit, FrequencySpectrum,
};

use crate::util::hash_str;

const ANALYZER_ID: u64 = hash_str(concat!(module_path!(), "::FFTAnalyzer"));

pub const FFT_WINDOW_SIZE: usize = 2048; // Power of 2 for FFT, good balance of frequency resolution vs latency

pub type SpectrumBuffer = Arc<ThingBuf<Arc<FrequencySpectrum>>>;

/// Custom AudioUnit that performs FFT analysis and activates sirens
pub struct FFTAnalyzer {
    inner_net: BigBlockAdapter,
    window_thb: ThingBuf<f32>,

    // Configuration and controls
    sample_rate: f32,
    config: Config,
    activation_controls: HashMap<NodeKey, Shared>,

    // Spectrum data storage
    spectrum_thb: SpectrumBuffer,
}

impl Clone for FFTAnalyzer {
    fn clone(&self) -> Self {
        Self {
            inner_net: self.inner_net.clone(),
            window_thb: ThingBuf::new(FFT_WINDOW_SIZE * 2),
            sample_rate: self.sample_rate,
            config: self.config.clone(),
            activation_controls: self.activation_controls.clone(),
            spectrum_thb: self.spectrum_thb.clone(),
        }
    }
}

impl FFTAnalyzer {
    pub fn new(
        inner_net: Box<dyn AudioUnit>,
        config: Config,
        activation_controls: HashMap<NodeKey, Shared>,
        spectrum_thb: Arc<ThingBuf<Arc<FrequencySpectrum>>>,
    ) -> Self {
        let analyzer = Self {
            inner_net: BigBlockAdapter::new(inner_net),
            window_thb: ThingBuf::new(FFT_WINDOW_SIZE * 2),
            sample_rate: config.sample_rate,
            config,
            activation_controls,
            spectrum_thb,
        };

        // Validate initial setup
        analyzer.validate_sensor_controls();
        analyzer
    }

    /// Validate that sensor data NodeKeys match available activation controls
    fn validate_sensor_controls(&self) {
        let missing_controls: Vec<NodeKey> = self
            .config
            .sensor_data
            .iter()
            .filter_map(|sensor| {
                if !self.activation_controls.contains_key(&sensor.key) {
                    Some(sensor.key)
                } else {
                    None
                }
            })
            .collect();

        if !missing_controls.is_empty() {
            log::warn!(
                "FFT analyzer missing activation controls for sensor keys: {:?}",
                missing_controls
            );
        }

        // Check for orphaned controls (controls without corresponding sensors)
        let sensor_keys: std::collections::HashSet<NodeKey> =
            self.config.sensor_data.iter().map(|s| s.key).collect();
        let orphaned_controls: Vec<NodeKey> = self
            .activation_controls
            .keys()
            .filter(|k| !sensor_keys.contains(k))
            .copied()
            .collect();

        if !orphaned_controls.is_empty() {
            log::warn!(
                "FFT analyzer has orphaned activation controls for keys: {:?}",
                orphaned_controls
            );
        }
    }

    /// Apply slow-growth activation function using x^3 curve
    /// This function grows very slowly from 0 to 1, making it less likely to hit 1.0
    fn slow_growth_activation(x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }

        // Use x^3 for slow growth
        // This gives:
        // x=0.1 -> 0.001
        // x=0.2 -> 0.008
        // x=0.3 -> 0.027
        // x=0.5 -> 0.125
        // x=0.7 -> 0.343
        // x=0.9 -> 0.729
        x * x * x
    }

    fn perform_fft_analysis(&mut self, window: &[f32], sensor_inputs: &[f32]) {
        log::trace!(
            "fft_analyzer: perform_fft_analysis enter sample_rate={}",
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

        // Process sensor activations
        for (i, sensor) in self.config.sensor_data.iter().enumerate() {
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

            let center = 0.5 * (min_freq + max_freq);

            // Test min, center, and max frequencies
            let test_freqs = [min_freq, center, max_freq];

            let mut max_activation = 0.0f32;

            for &test_freq in &test_freqs {
                let (f, v) = spectrum.freq_val_closest(test_freq);
                let freq = f.val();
                let peak_db = v.val();

                // Check if the closest bin is within sensor band
                let in_band = freq >= min_freq && freq <= max_freq;

                // Calculate raw activation value
                let raw_activation = if !in_band || peak_db <= min_mag {
                    0.0
                } else {
                    // Linear interpolation based on min/max magnitude range
                    ((peak_db - min_mag) / (max_mag - min_mag)).clamp(0.0, 1.0)
                };

                // Apply slow-growth function
                let activation = Self::slow_growth_activation(raw_activation);
                max_activation = max_activation.max(activation);

                if log::log_enabled!(log::Level::Trace) && activation > 0.01 {
                    log::trace!(
                        "fft_analyzer: sensor key={:?} test_freq={:.1} Hz, closest={:.1} Hz, peak={:.1} dB, raw={:.3}, activation={:.3}",
                        sensor.key,
                        test_freq,
                        freq,
                        peak_db,
                        raw_activation,
                        activation
                    );
                }
            }

            if log::log_enabled!(log::Level::Trace) && max_activation > 0.01 {
                log::trace!(
                    "fft_analyzer: sensor key={:?} range=[{:.1},{:.1}] final_activation={:.3}",
                    sensor.key,
                    min_freq,
                    max_freq,
                    max_activation
                );
            }

            // Update siren control with the maximum activation from all test points
            if let Some(siren_control) = self.activation_controls.get(&sensor.key) {
                siren_control.set_value(max_activation);
            } else {
                log::warn!(
                    "fft_analyzer: no siren control found for sensor key={:?}",
                    sensor.key
                );
            }
        }

        if let Err(full) = self.spectrum_thb.push(Arc::new(spectrum)) {
            log::debug!("fft_analyzer: failed to push spectrum data, cathing up");
            _ = self.spectrum_thb.pop();
            match self.spectrum_thb.push(full.into_inner()) {
                Ok(_) => {}
                Err(_) => {
                    log::error!("fft_analyzer: failed to push spectrum data after pop");
                }
            }
        }
    }

    fn consume_thb_window(&mut self, sensor_inputs: &[f32]) {
        if self.window_thb.len() >= FFT_WINDOW_SIZE {
            log::trace!(
                "fft_analyzer: consume_thb_window with {} samples available",
                self.window_thb.len()
            );
            let mut window = [0.0; FFT_WINDOW_SIZE];

            for frame in window.iter_mut() {
                if let Some(sample) = self.window_thb.pop() {
                    *frame = sample
                } else {
                    log::error!("fft_analyzer: insufficient samples in window_thb");
                    break;
                }
            }

            self.perform_fft_analysis(&window, sensor_inputs);
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
        // Process through inner network with all inputs
        self.inner_net.tick(&input[0..1], output);

        // Extract sensor inputs from additional channels
        let sensor_inputs = &input[1..];

        if self.window_thb.push(output[0]).is_err() {
            log::warn!("failed to push one sample")
        }

        self.consume_thb_window(sensor_inputs);
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

        'outer: for chunk in output.channel(0) {
            for s in chunk.as_array_ref() {
                if self.window_thb.push(*s).is_err() {
                    break 'outer;
                }
            }
        }

        self.consume_thb_window(&sensor_inputs);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner_net.set_sample_rate(sample_rate);
        self.sample_rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.inner_net.reset();
        while self.window_thb.pop().is_some() {}

        // Reset all siren controls
        for siren_control in self.activation_controls.values() {
            siren_control.set_value(0.0);
        }

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
        let control = shared(0.0);
        siren_controls.insert(NodeKey::new(0, 0), control.clone());

        let spectrum_thb = Arc::new(ThingBuf::new(10));

        // Create analyzer
        let analyzer = FFTAnalyzer::new(inner_net, config, siren_controls, spectrum_thb);

        // Test basic properties
        assert_eq!(analyzer.inputs(), 1);
        assert_eq!(analyzer.outputs(), 1);

        // Test initial values
        assert_eq!(control.value(), 0.0);
    }

    #[test]
    fn test_sensor_activation_threshold() {
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
        let control = shared(0.0);
        siren_controls.insert(NodeKey::new(0, 0), control.clone());

        let spectrum_thb = Arc::new(ThingBuf::new(10));

        let mut analyzer = FFTAnalyzer::new(inner_net, config, siren_controls, spectrum_thb);

        // Reset should clear everything
        analyzer.reset();
        assert_eq!(control.value(), 0.0);
    }

    #[test]
    fn test_slow_growth_activation() {
        // Test boundary conditions
        assert_eq!(FFTAnalyzer::slow_growth_activation(0.0), 0.0);
        assert_eq!(FFTAnalyzer::slow_growth_activation(1.0), 1.0);
        assert_eq!(FFTAnalyzer::slow_growth_activation(-0.1), 0.0);
        assert_eq!(FFTAnalyzer::slow_growth_activation(1.5), 1.0);

        // Test very small values (linear approximation region)
        let small_val = FFTAnalyzer::slow_growth_activation(0.005);
        assert!(small_val > 0.0 && small_val < 0.01);

        // Test slow growth property: function should grow slowly
        let test_points = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        let mut prev = 0.0;

        for &x in &test_points {
            let activation = FFTAnalyzer::slow_growth_activation(x);

            // Should be monotonically increasing
            assert!(
                activation > prev,
                "Activation should increase: {} > {}",
                activation,
                prev
            );

            // Should be bounded [0, 1]
            assert!((0.0..=1.0).contains(&activation));

            // Should be significantly less than linear (slower growth)
            assert!(
                activation < x * 0.9,
                "Activation {} should be much less than linear {}",
                activation,
                x * 0.9
            );

            prev = activation;
        }

        // Near x=1, should approach 1 but slowly
        let near_one = FFTAnalyzer::slow_growth_activation(0.95);
        assert!(near_one > 0.8 && near_one < 1.0);

        println!("Slow-growth activation test values:");
        for x in [
            0.0, 0.01, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0,
        ] {
            println!(
                "  f({:.2}) = {:.4}",
                x,
                FFTAnalyzer::slow_growth_activation(x)
            );
        }
    }
}
