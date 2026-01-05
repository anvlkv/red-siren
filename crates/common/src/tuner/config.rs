use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::tuner::layout::Layout;
use crate::{NodeKey, NodeKeyRegistry};

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub sensor_data: Vec<SensorData>,
    pub sample_rate: f32,
    pub fft_size: usize,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorData {
    pub key: NodeKey,
    pub min_frequency: f32, // Hz
    pub max_frequency: f32, // Hz
    pub min_magnitude: f32, // 0.0 scaled
    pub max_magnitude: f32, // 1.0 scaled
}

impl Config {
    pub fn new(
        tuner_layout: Layout,
        sample_rate: f32,
        fft_size: usize,
        registry: NodeKeyRegistry,
    ) -> Self {
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

        let cfg = Self {
            sensor_data: vec![],
            sample_rate,
            fft_size,
        };

        // Generate logarithmically spaced frequency ranges
        let sensor_data = cfg.generate_default_sensors(&registry);

        Self { sensor_data, ..cfg }
    }

    pub fn frequency_magnitude_to_space(
        &self,
        layout: &Layout,
        frequency: f32,
        mag_norm: f32,
    ) -> mint::Point2<f64> {
        let mag_norm = mag_norm.clamp(0.0, 1.0) as f64;
        let freq_clamped = frequency.clamp(self.min_freq(), self.max_freq()) as f64;
        let freq_range = (self.max_freq() as f64) - (self.min_freq() as f64);
        let freq_ratio = ((freq_clamped - (self.min_freq() as f64)) / freq_range).clamp(0.0, 1.0);

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
        let freq_range = (self.max_freq() as f64) - (self.min_freq() as f64);
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

        let frequency = ((freq_ratio * freq_range) + (self.min_freq() as f64))
            .clamp(self.min_freq() as f64, self.max_freq() as f64);

        (frequency as f32, mag_norm as f32)
    }

    pub fn min_freq(&self) -> f32 {
        self.sample_rate / (self.fft_size as f32)
    }

    pub fn max_freq(&self) -> f32 {
        self.sample_rate / 2.0
    }

    /// Generate default sensors with linear (even) frequency spacing across [min_freq, max_freq].
    pub fn generate_default_sensors(&self, registry: &NodeKeyRegistry) -> Vec<SensorData> {
        let total_keys = registry.total_keys();
        let mut sensors = Vec::with_capacity(total_keys);

        let min_freq = self.min_freq();
        let max_freq = self.max_freq();
        let total = total_keys as f32;
        let step = if total_keys > 0 {
            (max_freq - min_freq) / total
        } else {
            0.0
        };

        // Default magnitude thresholds
        let default_min_magnitude = 0.015;
        let default_max_magnitude = 0.75;

        // Generate sensors for each key using registry order
        let mut sensor_idx = 0;
        registry.iter_keys(|node_key| {
            if sensor_idx < total_keys {
                let sensor_min_freq = min_freq + (sensor_idx as f32) * step;
                let sensor_max_freq = min_freq + ((sensor_idx + 1) as f32) * step;

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
}

#[cfg(test)]
mod tests {
    use crate::instrument::layout_test_cases;
    use insta::assert_json_snapshot;

    use super::*;

    #[test]
    fn tuner_config() {
        for layout in layout_test_cases() {
            let tuner_layout: Layout = layout.into();
            let config = Config::new(tuner_layout, 44100.0, 2048, layout.registry());

            assert_json_snapshot!(
                format!("tuner_config_{}x{}", layout.space.x, layout.space.y),
                config
            )
        }
    }
}
