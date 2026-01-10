use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::tuner::layout::Layout;
use crate::tuner::ReflectTunerConstraints;
use crate::{NodeKey, NodeKeyRegistry};

const DEFAULT_INPUT_NY_THRESHOLD: f32 = 0.75;
const DEFAULT_INPUT_NY_WET_RATIO: f32 = 0.3;

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sensor_data: Vec<SensorData>,
    pub sample_rate: f32,
    pub fft_size: usize,
    pub ny_threshold: f32,
    pub ny_wet_ratio: f32,
    pub frequency_range: (Option<f32>, Option<f32>),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sensor_data: vec![],
            sample_rate: 44100.0,
            fft_size: 2048,
            ny_threshold: DEFAULT_INPUT_NY_THRESHOLD,
            ny_wet_ratio: DEFAULT_INPUT_NY_WET_RATIO,
            frequency_range: (None, None),
        }
    }
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
    pub fn new_from_previous(
        tuner_layout: Layout,
        sample_rate: f32,
        fft_size: usize,
        registry: NodeKeyRegistry,
        previous: &Config,
    ) -> Self {
        let mut cfg = Self::new(tuner_layout, sample_rate, fft_size, registry);

        cfg.ny_threshold = previous.ny_threshold;
        cfg.ny_wet_ratio = previous.ny_wet_ratio;

        cfg.frequency_range = (
            previous
                .frequency_range
                .0
                .filter(|r| cfg.min_freq_limit() < *r),
            previous
                .frequency_range
                .1
                .filter(|r| cfg.max_freq_limit() > *r),
        );

        let (min_lim, max_lim) = cfg.frequency_range_limits();

        // Attempt to preserve previous sensor settings where possible
        for sensor in cfg.sensor_data.iter_mut() {
            if let Some(old_value) = previous
                .sensor_data
                .iter()
                .find(|s| s.key == sensor.key)
                .copied()
            {
                sensor.min_frequency = old_value.min_frequency.clamp(min_lim, max_lim);
                sensor.max_frequency = old_value.max_frequency.clamp(min_lim, max_lim);
                sensor.min_magnitude = old_value.min_magnitude;
                sensor.max_magnitude = old_value.max_magnitude;
            }
        }

        cfg
    }

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
            ..Default::default()
        };

        // Generate default frequency ranges
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
        let (min_freq, max_freq) = self.frequency_range_limits();
        let freq_clamped = frequency.clamp(min_freq, max_freq) as f64;
        let freq_range = (max_freq as f64) - (min_freq as f64);
        let freq_ratio = ((freq_clamped - (min_freq as f64)) / freq_range).clamp(0.0, 1.0);

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
        let (min_freq, max_freq) = self.frequency_range_limits();
        let freq_range = (max_freq as f64) - (min_freq as f64);
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

        let frequency =
            ((freq_ratio * freq_range) + (min_freq as f64)).clamp(min_freq as f64, max_freq as f64);

        (frequency as f32, mag_norm as f32)
    }

    pub fn min_freq_limit(&self) -> f32 {
        self.sample_rate / (self.fft_size as f32)
    }

    pub fn max_freq_limit(&self) -> f32 {
        self.sample_rate / 2.0
    }

    pub fn frequency_range_limits(&self) -> (f32, f32) {
        (
            self.frequency_range
                .0
                .unwrap_or_else(|| self.min_freq_limit()),
            self.frequency_range
                .1
                .unwrap_or_else(|| self.max_freq_limit()),
        )
    }

    pub fn update_frequency_range(
        &mut self,
        min_frequency: Option<f32>,
        max_frequency: Option<f32>,
    ) {
        self.frequency_range.0 =
            min_frequency.filter(|&min_frequency| self.min_freq_limit() < min_frequency);
        self.frequency_range.1 =
            max_frequency.filter(|&max_frequency| self.max_freq_limit() > max_frequency);

        let (min_lim, max_lim) = self.frequency_range_limits();

        for sensor in self.sensor_data.iter_mut() {
            sensor.min_frequency = sensor.min_frequency.clamp(min_lim, max_lim);
            sensor.max_frequency = sensor.max_frequency.clamp(min_lim, max_lim);
        }
    }

    pub fn constraints(&self) -> ReflectTunerConstraints {
        ReflectTunerConstraints {
            min_frequency: self.frequency_range.0,
            max_frequency: self.frequency_range.1,
            frequency_limit: (self.min_freq_limit(), self.max_freq_limit()),
            ny_threshold: self.ny_threshold,
            wet_ratio: self.ny_wet_ratio,
        }
    }

    /// Generate default sensors with linear (even) frequency spacing across [min_freq, max_freq].
    pub fn generate_default_sensors(&self, registry: &NodeKeyRegistry) -> Vec<SensorData> {
        let total_keys = registry.total_keys();
        let mut sensors = Vec::with_capacity(total_keys);

        let min_freq = self.min_freq_limit();
        let max_freq = self.max_freq_limit();
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
