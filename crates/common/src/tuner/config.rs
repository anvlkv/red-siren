use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::tuner::layout::Layout;
use crate::{NodeKey, NodeKeyRegistry};

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub sensor_data: Vec<SensorData>,
    pub sample_rate: f32,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorData {
    pub key: NodeKey,
    pub min_frequency: f32, // Hz
    pub max_frequency: f32, // Hz
    pub min_magnitude: f32, // dB (20*log10) threshold
    pub max_magnitude: f32, // dB (20*log10) threshold
}

impl Config {
    pub fn new(tuner_layout: Layout, sample_rate: f32, registry: NodeKeyRegistry) -> Self {
        // Calculate total keys from layout - ensure consistency
        let total_keys = registry.total_keys();

        // Validate total_keys matches tuner layout
        if total_keys != tuner_layout.num_sensors.get() as usize {
            log::warn!(
                "Tuner layout total keys mismatch: registry={}, layout={}",
                total_keys,
                tuner_layout.num_sensors.get()
            );
        }

        // Generate logarithmically spaced frequency ranges
        let sensor_data = generate_default_sensors(total_keys, &registry);

        Self {
            sensor_data,
            sample_rate,
        }
    }

    pub fn frequency_magnitude_to_space(
        &self,
        layout: &Layout,
        frequency: f32,
        magnitude: f32,
    ) -> mint::Point2<f64> {
        // Normalize inputs (log-frequency mapping: 20Hz..Nyquist)
        let nyquist = (self.sample_rate / 2.0) as f64;
        let f_min = 20.0_f64;
        let log_min = f_min.ln();
        let log_max = nyquist.ln();
        let freq_ratio =
            (((frequency as f64).max(f_min).ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0);
        let min_db = -120.0_f64;
        let max_db = 0.0_f64;
        let mag_norm = (((magnitude as f64) - min_db) / (max_db - min_db)).clamp(0.0, 1.0);

        // Anchor to baseline with sensor-radius margins and full perpendicular range
        let (start, end) = layout.line_position;
        let r = layout.sensor_radius;
        let inset = r + 0.5;

        match layout.orientation {
            crate::orientation::LayoutOrientation::Horizontal => {
                // Along baseline: left -> right, inset by radius on both ends
                let line_len = end.x - start.x;
                let x = (start.x + inset) + freq_ratio * (line_len - 2.0 * inset);

                // Perpendicular: from baseline upward to top
                let avail_up = start.y;
                let y = start.y - mag_norm * avail_up;

                Point2 { x, y }
            }
            crate::orientation::LayoutOrientation::Vertical => {
                // Along baseline: top -> bottom, inset by radius on both ends
                let line_len = end.y - start.y;
                let y = (start.y + inset) + freq_ratio * (line_len - 2.0 * inset);

                // Perpendicular: from baseline rightward to screen edge
                let avail_right = layout.space.x - start.x;
                let x = start.x + mag_norm * avail_right;

                Point2 { x, y }
            }
        }
    }

    pub fn space_to_frequency_magnitude(
        &self,
        layout: &Layout,
        point: mint::Point2<f64>,
    ) -> (f32, f32) {
        let nyquist = (self.sample_rate / 2.0) as f64;
        let (start, end) = layout.line_position;
        let r = layout.sensor_radius;
        let inset = r + 0.5;

        let (freq_ratio, mag_norm) = match layout.orientation {
            crate::orientation::LayoutOrientation::Horizontal => {
                // Along baseline with radius margins
                let line_len = end.x - start.x;
                let freq_ratio =
                    ((point.x - (start.x + inset)) / (line_len - 2.0 * inset)).clamp(0.0, 1.0);

                // Perpendicular: baseline upward to top
                let avail_up = start.y;
                let mag_norm = ((start.y - point.y) / avail_up).clamp(0.0, 1.0);

                (freq_ratio, mag_norm)
            }
            crate::orientation::LayoutOrientation::Vertical => {
                // Along baseline with radius margins
                let line_len = end.y - start.y;
                let freq_ratio =
                    ((point.y - (start.y + inset)) / (line_len - 2.0 * inset)).clamp(0.0, 1.0);

                // Perpendicular: baseline rightward to screen edge
                let avail_right = layout.space.x - start.x;
                let mag_norm = ((point.x - start.x) / avail_right).clamp(0.0, 1.0);

                (freq_ratio, mag_norm)
            }
        };

        let f_min = 20.0_f64;
        let log_min = f_min.ln();
        let log_max = nyquist.ln();
        let frequency = (log_min + freq_ratio * (log_max - log_min)).exp();
        let min_db = -120.0_f64;
        let max_db = 0.0_f64;
        let magnitude = min_db + mag_norm * (max_db - min_db);

        (frequency as f32, magnitude as f32)
    }
}

/// Generate default sensors with logarithmic frequency spacing
fn generate_default_sensors(total_keys: usize, registry: &NodeKeyRegistry) -> Vec<SensorData> {
    let mut sensors = Vec::with_capacity(total_keys);

    // Frequency range for sensors (20Hz to 20kHz covers human hearing)
    let min_freq = 20.0_f32;
    let max_freq = 20000.0_f32;

    // Calculate logarithmic spacing
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();
    let log_step = (log_max - log_min) / (total_keys as f32);

    // Default magnitude thresholds (dB)
    let default_min_magnitude = -80.0;
    let default_max_magnitude = -20.0;

    // Generate sensors for each key using registry
    let mut sensor_idx = 0;
    registry.iter_keys(|node_key| {
        if sensor_idx < total_keys {
            // Calculate frequency range for this sensor
            let log_freq_start = log_min + (sensor_idx as f32) * log_step;
            let log_freq_end = log_min + ((sensor_idx + 1) as f32) * log_step;

            let sensor_min_freq = log_freq_start.exp();
            let sensor_max_freq = log_freq_end.exp();

            sensors.push(SensorData {
                key: node_key,
                min_frequency: sensor_min_freq,
                max_frequency: sensor_max_freq,
                min_magnitude: default_min_magnitude,
                max_magnitude: default_max_magnitude,
            });

            sensor_idx += 1;
        }
    });

    sensors
}
