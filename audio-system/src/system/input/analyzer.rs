use std::collections::HashMap;

use common::{tuner::Config, NodeKey};
use fundsp::hacker32::prelude::*;
use spectrum_analyzer::{samples_fft_to_spectrum, windows::hann_window, FrequencyLimit};

use crate::util::hash_str;

const ANALYZER_ID: u64 = hash_str(concat!(module_path!(), "::FFTAnalyzer"));
const ACTIVATION_THRESHOLD: f32 = 0.1; // Minimum magnitude to consider for activation
const ACTIVATION_SMOOTHING: f32 = 0.15; // Smoothing factor for activation updates

pub const FFT_WINDOW_SIZE: usize = 2048; // Power of 2 for FFT, good balance of frequency resolution vs latency

/// Custom AudioUnit that performs FFT analysis and activates sirens
#[derive(Clone)]
pub struct FFTAnalyzer {
    inner_net: Box<dyn AudioUnit>,
    window_buffer: Vec<f32>,
    window_size: usize,
    sample_count: usize,

    // Analysis results
    // peak_frequency: Arc<Shared>,
    // spectral_centroid: Arc<Shared>,
    // rms_level: Arc<Shared>,

    // Configuration and controls
    sample_rate: f32,
    config: Config,
    siren_controls: HashMap<NodeKey, Shared>,

    // Smoothed activation values for each sensor
    sensor_activations: HashMap<NodeKey, f32>,

    // Spectrum data storage
    spectrum_magnitudes: Vec<f32>,
    spectrum_frequencies: Vec<f32>,
}

impl FFTAnalyzer {
    pub fn new(
        inner_net: Box<dyn AudioUnit>,
        window_size: usize,
        config: Config,
        siren_controls: HashMap<NodeKey, Shared>,
    ) -> Self {
        // let peak_frequency = Arc::new(shared(0.0));
        // let spectral_centroid = Arc::new(shared(0.0));
        // let rms_level = Arc::new(shared(0.0));

        let mut sensor_activations = HashMap::new();
        for sensor in &config.sensor_data {
            sensor_activations.insert(sensor.key, 0.0);
        }

        Self {
            inner_net,
            window_buffer: vec![0.0; window_size],
            window_size,
            sample_count: 0,
            // peak_frequency: peak_frequency.clone(),
            // spectral_centroid: spectral_centroid.clone(),
            // rms_level: rms_level.clone(),
            sample_rate: config.sample_rate,
            config,
            siren_controls,
            sensor_activations,
            spectrum_magnitudes: Vec::new(),
            spectrum_frequencies: Vec::new(),
        }
    }

    /// Get the latest spectrum data (frequencies and magnitudes)
    pub fn get_spectrum_data(&self) -> Option<(Vec<f32>, Vec<f32>)> {
        if self.spectrum_magnitudes.is_empty() {
            None
        } else {
            Some((
                self.spectrum_frequencies.clone(),
                self.spectrum_magnitudes.clone(),
            ))
        }
    }

    /// Get current sensor activation levels
    pub fn get_sensor_activations(&self) -> HashMap<NodeKey, f32> {
        self.sensor_activations.clone()
    }

    fn perform_fft_analysis(&mut self) {
        // Apply Hann window
        let windowed = hann_window(&self.window_buffer);

        // Perform FFT using spectrum-analyzer
        let spectrum = match samples_fft_to_spectrum(
            &windowed,
            self.sample_rate as u32,
            FrequencyLimit::All,
            Some(&spectrum_analyzer::scaling::divide_by_N),
        ) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                log::error!("FFT analysis failed: {}", e);
                return;
            }
        };

        // Get spectrum data
        let spectrum_data = spectrum.data();

        // Store spectrum data for external access
        self.spectrum_magnitudes.clear();
        self.spectrum_frequencies.clear();
        for (frequency, magnitude) in spectrum_data {
            self.spectrum_frequencies.push(frequency.val());
            self.spectrum_magnitudes.push(magnitude.val());
        }

        // Process sensor activations
        for sensor in &self.config.sensor_data {
            let mut sensor_activation = 0.0;

            // Check spectrum data for frequencies within sensor range
            for (frequency, magnitude) in spectrum_data {
                let freq = frequency.val();
                let mag = magnitude.val();

                // Check if frequency falls within sensor range
                if freq >= sensor.min_frequency && freq <= sensor.max_frequency {
                    // Check if magnitude exceeds minimum threshold
                    if mag >= sensor.min_magnitude {
                        // Calculate normalized activation (0-1)
                        let activation = if sensor.max_magnitude > sensor.min_magnitude {
                            ((mag - sensor.min_magnitude)
                                / (sensor.max_magnitude - sensor.min_magnitude))
                                .clamp(0.0, 1.0)
                        } else if mag >= sensor.min_magnitude {
                            1.0
                        } else {
                            0.0
                        };

                        // Use maximum activation across the frequency range
                        sensor_activation = sensor_activation.max(activation);
                    }
                }
            }

            // Apply smoothing to prevent jitter
            let current_activation = self
                .sensor_activations
                .get(&sensor.key)
                .copied()
                .unwrap_or(0.0);
            let smoothed_activation = current_activation
                + (sensor_activation - current_activation) * ACTIVATION_SMOOTHING;

            // Update stored activation
            self.sensor_activations
                .insert(sensor.key, smoothed_activation);

            // Update siren control if we have a control for this sensor
            if let Some(siren_control) = self.siren_controls.get(&sensor.key) {
                // Only activate if above threshold
                if smoothed_activation > ACTIVATION_THRESHOLD {
                    siren_control.set_value(smoothed_activation);
                } else {
                    siren_control.set_value(0.0);
                }
            }
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
            self.window_buffer[self.sample_count % self.window_size] = output[0];
            self.sample_count += 1;

            // Perform FFT analysis when buffer is full
            if self.sample_count.is_multiple_of(self.window_size) {
                self.perform_fft_analysis();
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
                self.window_buffer[self.sample_count % self.window_size] = sample;
                self.sample_count += 1;

                // Perform FFT analysis when buffer is full
                if self.sample_count.is_multiple_of(self.window_size) {
                    self.perform_fft_analysis();
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
        self.window_buffer.fill(0.0);

        // Reset all sensor activations
        for activation in self.sensor_activations.values_mut() {
            *activation = 0.0;
        }

        // Reset all siren controls
        for siren_control in self.siren_controls.values() {
            siren_control.set_value(0.0);
        }

        // Clear spectrum data
        self.spectrum_magnitudes.clear();
        self.spectrum_frequencies.clear();
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
