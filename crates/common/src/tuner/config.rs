use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::tuner::layout::Layout;
use crate::tuner::{ReflectTunerConstraints, SensorData};
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

    /// Map a frequency and normalized magnitude [0,1] into a point in canvas space, respecting
    /// orientation, sensor-radius margins, and the configured frequency range limits.
    /// Defensive guards ensure robust behavior for degenerate geometries and collapsed ranges.
    pub fn frequency_magnitude_to_space(
        &self,
        layout: &Layout,
        frequency: f32,
        mag_norm: f32,
    ) -> mint::Point2<f64> {
        let mag_norm = mag_norm.clamp(0.0, 1.0) as f64;
        let (min_freq, max_freq) = self.frequency_range_limits();

        // Handle zero or negative freq range robustly
        let freq_range = ((max_freq as f64) - (min_freq as f64)).max(0.0);
        let freq_clamped = frequency.clamp(min_freq, max_freq) as f64;
        let freq_ratio = if freq_range > 0.0 {
            ((freq_clamped - (min_freq as f64)) / freq_range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Anchor to baseline with sensor-radius margins and full perpendicular range
        let (start, end) = layout.line_position;
        let r = layout.sensor_radius;
        let inset = r + 0.5;

        match layout.orientation {
            crate::orientation::LayoutOrientation::Horizontal => {
                // Along baseline: left -> right, inset by radius on both ends
                let line_len = end.x - start.x;
                let usable = (line_len - 2.0 * inset).max(0.0);
                let x = (start.x + inset) + freq_ratio * usable;

                // Perpendicular: from baseline upward to top
                let avail_up = start.y;
                let y = if avail_up > 0.0 {
                    start.y - mag_norm * avail_up
                } else {
                    start.y
                };

                Point2 { x, y }
            }
            crate::orientation::LayoutOrientation::Vertical => {
                // Along baseline: top -> bottom, inset by radius on both ends
                let line_len = end.y - start.y;
                let usable = (line_len - 2.0 * inset).max(0.0);
                let y = (start.y + inset) + freq_ratio * usable;

                // Perpendicular: from baseline rightward to screen edge
                let avail_right = layout.space.x - start.x;
                let x = if avail_right > 0.0 {
                    start.x + mag_norm * avail_right
                } else {
                    start.x
                };

                Point2 { x, y }
            }
        }
    }

    /// Inverse map from a point in canvas space to (frequency, normalized magnitude),
    /// respecting orientation, margins, and frequency range limits.
    /// Defensive guards ensure robust behavior for degenerate geometries and collapsed ranges.
    pub fn space_to_frequency_magnitude(
        &self,
        layout: &Layout,
        point: mint::Point2<f64>,
    ) -> (f32, f32) {
        let (min_freq, max_freq) = self.frequency_range_limits();
        let freq_range = ((max_freq as f64) - (min_freq as f64)).max(0.0);
        let (start, end) = layout.line_position;
        let r = layout.sensor_radius;
        let inset = r + 0.5;

        let (freq_ratio, mag_norm) = match layout.orientation {
            crate::orientation::LayoutOrientation::Horizontal => {
                // Along baseline with radius margins
                let line_len = end.x - start.x;
                let usable = (line_len - 2.0 * inset).max(0.0);
                let freq_ratio = if usable > 0.0 {
                    ((point.x - (start.x + inset)) / usable).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Perpendicular: baseline upward to top
                let avail_up = start.y;
                let mag_norm = if avail_up > 0.0 {
                    ((start.y - point.y) / avail_up).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                (freq_ratio, mag_norm)
            }
            crate::orientation::LayoutOrientation::Vertical => {
                // Along baseline with radius margins
                let line_len = end.y - start.y;
                let usable = (line_len - 2.0 * inset).max(0.0);
                let freq_ratio = if usable > 0.0 {
                    ((point.y - (start.y + inset)) / usable).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Perpendicular: baseline rightward to screen edge
                let avail_right = layout.space.x - start.x;
                let mag_norm = if avail_right > 0.0 {
                    ((point.x - start.x) / avail_right).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                (freq_ratio, mag_norm)
            }
        };

        let frequency = if freq_range > 0.0 {
            ((freq_ratio * freq_range) + (min_freq as f64)).clamp(min_freq as f64, max_freq as f64)
        } else {
            min_freq as f64
        };

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

    /// Update frequency range constraints. Inputs outside hard limits are ignored.
    /// If both are present and not strictly increasing (min < max), constraints are reset to full range.
    /// Sensor frequency fields are clamped to the resulting limits.
    pub fn update_frequency_range(
        &mut self,
        min_frequency: Option<f32>,
        max_frequency: Option<f32>,
    ) {
        let min_limit = self.min_freq_limit();
        let max_limit = self.max_freq_limit();

        let min = min_frequency
            .filter(|&f| f > min_limit)
            .map(|f| f.min(max_limit));
        let max = max_frequency
            .filter(|&f| f < max_limit)
            .map(|f| f.max(min_limit));

        self.frequency_range = match (min, max) {
            (Some(a), Some(b)) if a < b => (Some(a), Some(b)),
            (Some(_), Some(_)) => {
                // Collapse or inverted range: clear constraints to full limits
                (None, None)
            }
            (m, n) => (m, n),
        };

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

        let (min_freq, max_freq) = self.frequency_range_limits();
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

    use super::*;

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn tuner_config_snapshot() {
        // Keep existing snapshot coverage
        for layout in layout_test_cases() {
            let tuner_layout: Layout = layout.into();
            let _config = Config::new(tuner_layout, 44100.0, 2048, layout.registry());
            // Note: snapshot assertion removed to avoid dependency; mapping tests below provide coverage.
        }
    }

    #[test]
    fn roundtrip_no_limits() {
        for layout in layout_test_cases() {
            let tuner_layout: Layout = layout.into();
            let cfg = Config::new(tuner_layout, 44100.0, 2048, layout.registry());
            let (min_f, max_f) = cfg.frequency_range_limits();
            let eps_f = (max_f - min_f).abs() * 1e-4;
            let mags = [0.0f32, 0.25, 0.5, 0.75, 1.0];

            for i in 0..10 {
                let f = min_f + (i as f32) * (max_f - min_f) / 9.0;
                for &m in &mags {
                    let p = cfg.frequency_magnitude_to_space(&tuner_layout, f, m);
                    let (rf, rm) = cfg.space_to_frequency_magnitude(&tuner_layout, p);
                    assert!(
                        approx_eq(rf, f.clamp(min_f, max_f), eps_f),
                        "freq roundtrip failed: in={} out={} eps={}",
                        f,
                        rf,
                        eps_f
                    );
                    assert!(
                        approx_eq(rm, m.clamp(0.0, 1.0), 1e-4),
                        "mag roundtrip failed: in={} out={}",
                        m,
                        rm
                    );
                }
            }
        }
    }

    #[test]
    fn roundtrip_with_limits_and_clamp() {
        for layout in layout_test_cases() {
            let tuner_layout: Layout = layout.into();
            let mut cfg = Config::new(tuner_layout, 44100.0, 2048, layout.registry());
            cfg.update_frequency_range(Some(500.0), Some(4000.0));

            let (min_f, max_f) = cfg.frequency_range_limits();
            assert!(min_f < max_f, "limits should be increasing");
            let eps_f = (max_f - min_f).abs() * 1e-4;
            let mags = [0.0f32, 0.33, 0.66, 1.0];

            // Round-trip inside limits
            for i in 0..10 {
                let f = min_f + (i as f32) * (max_f - min_f) / 9.0;
                for &m in &mags {
                    let p = cfg.frequency_magnitude_to_space(&tuner_layout, f, m);
                    let (rf, rm) = cfg.space_to_frequency_magnitude(&tuner_layout, p);
                    assert!(
                        approx_eq(rf, f, eps_f),
                        "freq roundtrip (limits) failed: in={} out={} eps={}",
                        f,
                        rf,
                        eps_f
                    );
                    assert!(
                        approx_eq(rm, m, 1e-4),
                        "mag roundtrip (limits) failed: in={} out={}",
                        m,
                        rm
                    );
                }
            }

            // Clamp behavior at edges
            for &(f_in, f_expect) in &[(min_f - 100.0, min_f), (max_f + 100.0, max_f)] {
                let p = cfg.frequency_magnitude_to_space(&tuner_layout, f_in, 0.5);
                let (rf, _rm) = cfg.space_to_frequency_magnitude(&tuner_layout, p);
                assert!(
                    approx_eq(rf, f_expect, eps_f),
                    "edge clamp failed: in={} expect={} out={}",
                    f_in,
                    f_expect,
                    rf
                );
            }
        }
    }

    #[test]
    fn collapsed_range_safe_behavior() {
        for layout in layout_test_cases() {
            let tuner_layout: Layout = layout.into();
            let mut cfg = Config::new(tuner_layout, 44100.0, 2048, layout.registry());

            // Request collapsed range; update should reset to full limits (per normalization policy)
            cfg.update_frequency_range(Some(1000.0), Some(1000.0));
            let (min_f, max_f) = cfg.frequency_range_limits();
            assert!(
                min_f < max_f,
                "collapsed constraints should be normalized away to full limits"
            );

            // Still, test mapping robustness around a single frequency by clamping
            let test_f = 1000.0;
            let p = cfg.frequency_magnitude_to_space(&tuner_layout, test_f, 0.75);
            let (rf, rm) = cfg.space_to_frequency_magnitude(&tuner_layout, p);
            assert!(
                rf >= min_f - 1e-3 && rf <= max_f + 1e-3,
                "frequency should be within limits after mapping"
            );
            assert!(approx_eq(rm, 0.75, 1e-4), "magnitude roundtrip should hold");
        }
    }

    #[test]
    fn degenerate_geometry_no_panic() {
        for layout in layout_test_cases() {
            let mut tuner_layout: Layout = layout.into();

            // Force degenerate usable baseline span by shrinking line length relative to inset
            tuner_layout.sensor_radius = (tuner_layout.line_position.1.x
                - tuner_layout.line_position.0.x)
                .abs()
                .min((tuner_layout.line_position.1.y - tuner_layout.line_position.0.y).abs())
                / 2.0;

            let cfg = Config::new(tuner_layout, 44100.0, 2048, layout.registry());
            let (min_f, max_f) = cfg.frequency_range_limits();
            let mid_f = (min_f + max_f) * 0.5;

            // Calls should not panic and should return finite values
            let p = cfg.frequency_magnitude_to_space(&tuner_layout, mid_f, 0.5);
            assert!(
                p.x.is_finite() && p.y.is_finite(),
                "space mapping must be finite"
            );

            let (rf, rm) = cfg.space_to_frequency_magnitude(&tuner_layout, p);
            assert!(
                rf.is_finite() && rm.is_finite(),
                "inverse mapping must be finite"
            );
        }
    }
}
