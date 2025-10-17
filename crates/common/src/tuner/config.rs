use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::tuner::layout::Layout;
use crate::NodeKey;

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub sensor_data: Vec<SensorData>,
    pub fft_size: usize,
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
    pub fn frequency_magnitude_to_space(
        &self,
        layout: &Layout,
        frequency: f32,
        magnitude: f32,
    ) -> mint::Point2<f32> {
        // Normalize inputs (log-frequency mapping: 20Hz..Nyquist)
        let nyquist = self.sample_rate / 2.0;
        let f_min = 20.0_f32;
        let log_min = f_min.ln();
        let log_max = nyquist.ln();
        let freq_ratio =
            ((frequency.max(f_min).ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0);
        let min_db = -120.0_f32;
        let max_db = 0.0_f32;
        let mag_norm = ((magnitude - min_db) / (max_db - min_db)).clamp(0.0, 1.0);

        // Anchor to baseline with sensor-radius margins and full perpendicular range
        let (start, end) = layout.line_position;
        let r = layout.sensor_radius;

        match layout.orientation {
            crate::orientation::LayoutOrientation::Horizontal => {
                // Along baseline: left -> right, inset by radius on both ends
                let line_len = end.x - start.x;
                let x = (start.x + r) + freq_ratio * (line_len - 2.0 * r);

                // Perpendicular: from baseline upward to top
                let avail_up = start.y;
                let y = start.y - mag_norm * avail_up;

                Point2 { x, y }
            }
            crate::orientation::LayoutOrientation::Vertical => {
                // Along baseline: top -> bottom, inset by radius on both ends
                let line_len = end.y - start.y;
                let y = (start.y + r) + freq_ratio * (line_len - 2.0 * r);

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
        point: mint::Point2<f32>,
    ) -> (f32, f32) {
        let nyquist = self.sample_rate / 2.0;
        let (start, end) = layout.line_position;
        let r = layout.sensor_radius;

        let (freq_ratio, mag_norm) = match layout.orientation {
            crate::orientation::LayoutOrientation::Horizontal => {
                // Along baseline with radius margins
                let line_len = end.x - start.x;
                let freq_ratio = ((point.x - (start.x + r)) / (line_len - 2.0 * r)).clamp(0.0, 1.0);

                // Perpendicular: baseline upward to top
                let avail_up = start.y;
                let mag_norm = ((start.y - point.y) / avail_up).clamp(0.0, 1.0);

                (freq_ratio, mag_norm)
            }
            crate::orientation::LayoutOrientation::Vertical => {
                // Along baseline with radius margins
                let line_len = end.y - start.y;
                let freq_ratio = ((point.y - (start.y + r)) / (line_len - 2.0 * r)).clamp(0.0, 1.0);

                // Perpendicular: baseline rightward to screen edge
                let avail_right = layout.space.x - start.x;
                let mag_norm = ((point.x - start.x) / avail_right).clamp(0.0, 1.0);

                (freq_ratio, mag_norm)
            }
        };

        let f_min = 20.0_f32;
        let log_min = f_min.ln();
        let log_max = nyquist.ln();
        let frequency = (log_min + freq_ratio * (log_max - log_min)).exp();
        let min_db = -120.0_f32;
        let max_db = 0.0_f32;
        let magnitude = min_db + mag_norm * (max_db - min_db);

        (frequency, magnitude)
    }
}

impl From<crate::instrument::Layout> for Config {
    fn from(layout: crate::instrument::Layout) -> Self {
        // Use existing Layout conversion
        let tuner_layout: Layout = layout.into();

        // Calculate total keys from layout
        let total_keys = tuner_layout.num_sensors.get() as usize;

        // Generate logarithmically spaced frequency ranges
        let sensor_data = generate_default_sensors(total_keys, &layout);

        Config {
            sensor_data,
            fft_size: 2048,
            sample_rate: 48000.0,
        }
    }
}

/// Generate default sensors with logarithmic frequency spacing
fn generate_default_sensors(
    total_keys: usize,
    layout: &crate::instrument::Layout,
) -> Vec<SensorData> {
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

    // Generate sensors for each key
    for group_idx in 0..layout.num_groups.get() {
        for key_idx in 0..layout.num_keys_per_group.get() {
            let sensor_idx = group_idx * layout.num_keys_per_group.get() + key_idx;

            // Calculate frequency range for this sensor
            let log_freq_start = log_min + (sensor_idx as f32) * log_step;
            let log_freq_end = log_min + ((sensor_idx + 1) as f32) * log_step;

            let sensor_min_freq = log_freq_start.exp();
            let sensor_max_freq = log_freq_end.exp();

            sensors.push(SensorData {
                key: NodeKey(group_idx, key_idx),
                min_frequency: sensor_min_freq,
                max_frequency: sensor_max_freq,
                min_magnitude: default_min_magnitude,
                max_magnitude: default_max_magnitude,
            });
        }
    }

    sensors
}
