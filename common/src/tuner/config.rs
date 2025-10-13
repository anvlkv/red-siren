use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};

use crate::tuner::layout::Layout;
use crate::NodeKey;

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub layout: Layout,
    pub sensor_data: Vec<SensorData>,
    pub fft_size: usize,
    pub sample_rate: f32,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorData {
    pub key: NodeKey,
    pub min_frequency: f32, // Hz
    pub max_frequency: f32, // Hz
    pub min_magnitude: f32, // Linear magnitude threshold
    pub max_magnitude: f32, // Linear magnitude threshold
}

impl Config {
    pub fn frequency_magnitude_to_space(
        &self,
        frequency: f32,
        magnitude: f32,
    ) -> mint::Point2<f32> {
        let Vector2 {
            x: space_width,
            y: space_height,
        } = self.layout.space;

        // Map frequency to 0-1 range based on sample rate
        let nyquist = self.sample_rate / 2.0;
        let freq_ratio = (frequency / nyquist).clamp(0.0, 1.0);

        // Magnitude is already normalized 0-1

        let (x, y) = match self.layout.orientation {
            crate::orientation::LayoutOrientation::Vertical => {
                let x = magnitude * space_width;
                let y = freq_ratio * space_height;
                (x, y)
            }
            crate::orientation::LayoutOrientation::Horizontal => {
                let x = freq_ratio * space_width;
                let y = magnitude * space_height;
                (x, y)
            }
        };

        Point2 { x, y }
    }

    pub fn space_to_frequency_magnitude(&self, point: mint::Point2<f32>) -> (f32, f32) {
        let Vector2 {
            x: space_width,
            y: space_height,
        } = self.layout.space;

        if space_width == 0.0 || space_height == 0.0 {
            return (0.0, 0.0);
        }

        let x = point.x;
        let y = point.y;

        let nyquist = self.sample_rate / 2.0;

        let (magnitude, freq_ratio) = match self.layout.orientation {
            crate::orientation::LayoutOrientation::Vertical => {
                let magnitude = (x / space_width).clamp(0.0, 1.0);
                let freq_ratio = (y / space_height).clamp(0.0, 1.0);
                (magnitude, freq_ratio)
            }
            crate::orientation::LayoutOrientation::Horizontal => {
                let freq_ratio = (x / space_width).clamp(0.0, 1.0);
                let magnitude = (y / space_height).clamp(0.0, 1.0);
                (magnitude, freq_ratio)
            }
        };

        let frequency = freq_ratio * nyquist;

        (frequency, magnitude)
    }
}

impl From<crate::instrument::Layout> for Config {
    fn from(layout: crate::instrument::Layout) -> Self {
        // Use existing Layout conversion
        let tuner_layout: Layout = layout.into();

        // Calculate total keys from layout
        let total_keys =
            (layout.num_groups.get() as usize) * (layout.num_keys_per_group.get() as usize);

        // Generate logarithmically spaced frequency ranges
        let sensor_data = generate_default_sensors(total_keys, &layout);

        Config {
            layout: tuner_layout,
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

    // Default magnitude thresholds
    let default_min_magnitude = 0.01;
    let default_max_magnitude = 0.8;

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
