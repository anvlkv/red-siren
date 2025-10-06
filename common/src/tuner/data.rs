//! Tuner runtime data module.
//!
//! Builds and owns spectral sensors derived from a `tuner::Layout` plus
//! static calibration (option 3: baked average gain per sensor).
//!
//! Design (Most Advanced Yet Acceptable):
//! - Partition the FFT frequency band of interest [MIN_FREQ_HZ, MAX_FREQ_HZ]
//!   into `num_sensors` contiguous sub‑bands (linear in frequency).
//! - For each sub‑band:
//!     * Determine FFT bin start/end (half‑spectrum, real FFT assumption).
//!     * Pre-compute average calibration gain (linear) across its bins using
//!       `gain_linear(freq)` from `tuner::config`.
//!     * Store spatial center & half-extents derived from layout.
//! - Runtime update:
//!     * Caller feeds the complex FFT output slice to `process_fft`.
//!     * Each sensor averages its bins (no dynamic per-bin calibration).
//!
//! If calibration curve changes or layout changes: rebuild `TunerData`.
//!
//! Future extensions (not implemented to keep simplicity):
//! - Log-spaced frequency partition
//! - Dynamic per-bin weighting (option 4)
//! - Temporal smoothing & decay
//! - Overlapping sensor bands
//!
//! Assumptions:
//! - `fft_size` provided matches the time-domain size used in the real FFT.
//! - Real FFT output length ≈ fft_size/2 + 1 (standard); caller supplies
//!   at least `max(bin_end)` complex bins when calling `process_fft`.
//!
//! Safety: if spectrum slice is shorter than expected in `process_fft`,
//! sensors that reference out-of-range bins will be silenced (activation=0).

use mint::{Point2, Vector2};
use num_complex::Complex32;

use crate::{
    instrument::consts::{MAX_FREQ_HZ, MIN_FREQ_HZ},
    tuner::{config::gain_linear, layout::Layout, sensor::Sensor},
};

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone)]
pub struct Data {
    pub layout: Layout,
    pub sample_rate: f32,
    pub fft_size: usize,
    pub sensors: Vec<Sensor>,
    /// Cached min & max bins used (inclusive start, exclusive end overall)
    pub global_bin_span: (usize, usize),
}

impl Data {
    /// Create a new tuner data instance.
    ///
    /// `fft_size` must match the size used with the real FFT.
    pub fn new(layout: Layout, sample_rate: f32, fft_size: usize) -> Self {
        let num_sensors = layout.num_sensors.get() as usize;
        assert!(num_sensors > 0, "Layout must define at least one sensor");
        assert!(fft_size >= 8, "fft_size unexpectedly small");

        // Half-spectrum usable bins for real FFT (excluding mirrored part)
        let half_bins = fft_size / 2; // Nyquist at index half_bins
        let min_bin = freq_to_bin(MIN_FREQ_HZ as f32, sample_rate, fft_size).min(half_bins);
        let mut max_bin = freq_to_bin(MAX_FREQ_HZ as f32, sample_rate, fft_size).min(half_bins);
        if max_bin <= min_bin {
            // Force range
            max_bin = (min_bin + 1).min(half_bins);
        }

        let total_bins_range = max_bin - min_bin;
        let bins_per_sensor = (total_bins_range as f32 / num_sensors as f32).max(1.0);

        // Spatial preparation
        let p0 = layout.line_position.0;
        let p1 = layout.line_position.1;
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;

        // Half-extents: convert layout.sensor_max_range according to orientation
        // Already oriented in Layout: main-axis length in x (Horizontal) else y (Vertical)
        let (half_x, half_y) = match layout.orientation {
            crate::orientation::LayoutOrientation::Horizontal => (
                layout.sensor_max_range.x * 0.5,
                layout.sensor_max_range.y * 0.5,
            ),
            crate::orientation::LayoutOrientation::Vertical => (
                layout.sensor_max_range.x * 0.5,
                layout.sensor_max_range.y * 0.5,
            ),
        };
        let half_extents = Vector2 {
            x: half_x,
            y: half_y,
        };

        let mut sensors = Vec::with_capacity(num_sensors);

        for i in 0..num_sensors {
            // Frequency / bin segmentation (linear)
            let seg_start_f = i as f32 * bins_per_sensor;
            let seg_end_f = (i as f32 + 1.0) * bins_per_sensor;
            let bin_start = min_bin + seg_start_f.floor() as usize;
            let mut bin_end = min_bin + seg_end_f.floor() as usize;
            if bin_end <= bin_start {
                bin_end = (bin_start + 1).min(max_bin);
            }
            if bin_end > max_bin {
                bin_end = max_bin;
            }

            // Spatial center along line
            let t = (i as f32 + 0.5) / num_sensors as f32;
            let center = Point2 {
                x: p0.x + dx * t,
                y: p0.y + dy * t,
            };

            // Average gain across this sensor's bins
            let avg_gain_linear =
                average_gain_over_bins(bin_start, bin_end, sample_rate, fft_size).unwrap_or(1.0);

            sensors.push(Sensor::new(
                i as u32,
                center,
                half_extents,
                bin_start,
                bin_end,
                avg_gain_linear,
            ));
        }

        let global_bin_span = (
            sensors.iter().map(|s| s.bin_start).min().unwrap_or(min_bin),
            sensors.iter().map(|s| s.bin_end).max().unwrap_or(max_bin),
        );

        Self {
            layout,
            sample_rate,
            fft_size,
            sensors,
            global_bin_span,
        }
    }

    /// Process a single FFT frame (complex half-spectrum).
    ///
    /// Expects at least `self.global_bin_span.1` bins in `spectrum`.
    pub fn process_fft(&mut self, spectrum: &[Complex32]) {
        if spectrum.len() < self.global_bin_span.1 {
            // Insufficient bins -> silence all
            for s in &mut self.sensors {
                s.update(&[]); // will zero
            }
            return;
        }
        for s in &mut self.sensors {
            s.update(spectrum);
        }
    }

    /// Access sensors (immutable).
    pub fn sensors(&self) -> &[Sensor] {
        &self.sensors
    }

    /// Access sensors (mutable) if caller wants custom processing.
    pub fn sensors_mut(&mut self) -> &mut [Sensor] {
        &mut self.sensors
    }

    /// Number of sensors.
    pub fn sensor_count(&self) -> usize {
        self.sensors.len()
    }

    /// Sample rate used for FFT interpretation.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// FFT size (time-domain length).
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// Underlying layout.
    pub fn layout(&self) -> Layout {
        self.layout
    }
}

/// Convert a frequency (Hz) to the nearest FFT bin index.
fn freq_to_bin(freq_hz: f32, sample_rate: f32, fft_size: usize) -> usize {
    if sample_rate <= 0.0 || fft_size == 0 {
        return 0;
    }
    let nyquist = sample_rate * 0.5;
    let f = freq_hz.clamp(0.0, nyquist);
    let bin_f = f * (fft_size as f32) / sample_rate;
    bin_f.round().clamp(0.0, (fft_size / 2) as f32) as usize
}

/// Convert a bin index back to frequency (Hz).
fn bin_to_freq(bin: usize, sample_rate: f32, fft_size: usize) -> f32 {
    if fft_size == 0 {
        return 0.0;
    }
    (bin as f32) * sample_rate / (fft_size as f32)
}

/// Average calibration gain across bin range.
/// Returns None if range empty or invalid.
fn average_gain_over_bins(
    bin_start: usize,
    bin_end: usize,
    sample_rate: f32,
    fft_size: usize,
) -> Option<f32> {
    if bin_end <= bin_start {
        return None;
    }
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for b in bin_start..bin_end {
        let f = bin_to_freq(b, sample_rate, fft_size) as f64;
        sum += gain_linear(f);
        count += 1;
    }
    if count == 0 {
        None
    } else {
        Some(sum / count as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::layout_test_cases;

    // A dummy complex helper
    fn c(re: f32) -> Complex32 {
        Complex32 { re, im: 0.0 }
    }

    #[test]
    fn build_tuner_data_from_instrument_layouts() {
        // Take first few instrument layouts and ensure tuner data builds & processes
        for layout in layout_test_cases().take(3) {
            let tuner_layout: crate::tuner::layout::Layout = layout.into();
            let mut data = Data::new(tuner_layout, 48_000.0, 2048);

            assert!(data.sensor_count() > 0);

            // Build a fake spectrum with sufficient bins
            let needed = data.global_bin_span.1;
            let mut spectrum = Vec::with_capacity(needed);
            for i in 0..needed {
                // simple gradient magnitude
                spectrum.push(c((i as f32 + 1.0) * 0.0005));
            }

            data.process_fft(&spectrum);

            // All activations should be finite & within [0,1]
            for s in data.sensors() {
                assert!(s.activation().is_finite());
                assert!((0.0..=1.0).contains(&s.activation()));
            }
        }
    }

    #[test]
    fn average_gain_non_zero() {
        let layout = layout_test_cases().next().unwrap();
        let tuner_layout: crate::tuner::layout::Layout = layout.into();
        let data = Data::new(tuner_layout, 44_100.0, 1024);
        for s in data.sensors() {
            assert!(s.avg_gain_linear > 0.0);
            assert!(s.avg_gain_linear.is_finite());
        }
    }
}
