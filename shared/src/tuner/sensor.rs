//! Tuner sensor module (Option 3: baked average gain per sensor)
//!
//! Each `Sensor`:
//! - Covers a contiguous FFT bin range `[bin_start, bin_end)`.
//! - Has a spatial representation (center + half_extents) in layout space for UI.
//! - Stores a pre–computed average calibration gain (`avg_gain_linear`) derived
//!   from the static calibration points (see `tuner::config`).
//! - Produces an activation ∈ [0.0, 1.0] from current FFT data.
//!
//! Gain Application Strategy (Chosen Design - Option 3):
//! ----------------------------------------------------
//! Calibration is baked at construction time into `avg_gain_linear`. Runtime
//! update multiplies the unweighted average magnitude of the covered bins by
//! this constant. If calibration points change, rebuild the sensors.
//!
//! Activation Mapping:
//! -------------------
//! 1. Collect average (linear) magnitude across bins
//! 2. Multiply by `avg_gain_linear`
//! 3. Convert to dB: 20 * log10(mag + EPS)
//! 4. Normalize from [FLOOR_DB, CEIL_DB] -> [0,1]
//!
//! This keeps runtime light and deterministic.
//!
//! If future needs require dynamic or per–bin gain, move the calibration to the
//! bin loop (Option 4) without changing the public interface.
//!
//! NOTE: `Sensor::update` is intentionally self–contained so higher–level code
//! can simply iterate sensors and call `update` with the FFT frame.
//!
//! Assumptions:
//! - `fft_bins` provided to `update` at runtime are at least `bin_end` long.
//! - Real FFT output ordering matches standard (DC..Nyquist).
//! - Caller ensures thread safety / non–aliasing between updates & reads.

use mint::{Point2, Vector2};
use num_complex::Complex32;

/// Small epsilon to avoid log10(0)
const EPS: f32 = 1.0e-9;
/// dB floor for normalization (anything below treated as silence)
const FLOOR_DB: f32 = -60.0;
/// dB ceiling (approx max expected average magnitude)
const CEIL_DB: f32 = 0.0;

/// Represents one layout + spectral sensor.
#[derive(Debug, Clone)]
pub struct Sensor {
    /// Zero-based sensor index
    pub index: u32,
    /// Center position in layout coordinate space
    pub center: Point2<f32>,
    /// Half extents of the sensor's spatial influence rectangle
    pub half_extents: Vector2<f32>,
    /// Inclusive start FFT bin
    pub bin_start: usize,
    /// Exclusive end FFT bin
    pub bin_end: usize,
    /// Baked average calibration gain (linear amplitude multiplier)
    pub avg_gain_linear: f32,
    /// Current activation (0.0 ..= 1.0)
    activation: f32,
}

impl Sensor {
    /// Create a new sensor.
    ///
    /// # Panics
    /// Panics if `bin_end <= bin_start`.
    pub fn new(
        index: u32,
        center: Point2<f32>,
        half_extents: Vector2<f32>,
        bin_start: usize,
        bin_end: usize,
        avg_gain_linear: f32,
    ) -> Self {
        assert!(bin_end > bin_start, "Empty or reversed bin range");
        Self {
            index,
            center,
            half_extents,
            bin_start,
            bin_end,
            avg_gain_linear: if avg_gain_linear.is_finite() && avg_gain_linear > 0.0 {
                avg_gain_linear
            } else {
                1.0
            },
            activation: 0.0,
        }
    }

    /// Current activation value.
    #[inline]
    pub fn activation(&self) -> f32 {
        self.activation
    }

    /// Update activation from provided FFT complex spectrum slice.
    ///
    /// `fft_bins` must contain at least `bin_end` elements.
    pub fn update(&mut self, fft_bins: &[Complex32]) {
        if self.bin_end > fft_bins.len() || self.bin_start >= self.bin_end {
            // Invalid slice; set to silence
            self.activation = 0.0;
            return;
        }

        // Average magnitude within range
        let mut sum_mag = 0.0f32;
        let mut count = 0usize;
        for c in &fft_bins[self.bin_start..self.bin_end] {
            // magnitude = sqrt(re^2 + im^2)
            sum_mag += c.re.hypot(c.im);
            count += 1;
        }

        if count == 0 {
            self.activation = 0.0;
            return;
        }

        let avg_mag = (sum_mag / count as f32) * self.avg_gain_linear;

        // Convert to dB
        let mag_db = 20.0 * (avg_mag + EPS).log10();

        // Normalize
        let norm = (mag_db - FLOOR_DB) / (CEIL_DB - FLOOR_DB); // denominator negative -> but CEIL_DB (0) - FLOOR_DB (-60) = 60 > 0
        self.activation = norm.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_bin(mag: f32) -> Complex32 {
        Complex32 { re: mag, im: 0.0 }
    }

    #[test]
    fn test_basic_activation() {
        // Create a sensor over bins [2,5)
        let mut s = Sensor::new(
            0,
            Point2 { x: 0.0, y: 0.0 },
            Vector2 { x: 1.0, y: 1.0 },
            2,
            5,
            2.0, // avg_gain_linear
        );

        // Build spectrum (at least 5 bins)
        let spectrum = vec![
            // bins 0..2 (ignored)
            synth_bin(0.1),
            synth_bin(0.1),
            // bins 2..5 (used)
            synth_bin(0.2),
            synth_bin(0.2),
            synth_bin(0.2),
            // extra bin
            synth_bin(0.0),
        ];

        s.update(&spectrum);

        // Activation should be finite and > 0
        assert!(s.activation().is_finite());
        assert!(s.activation() > 0.0);
        assert!(s.activation() <= 1.0);
    }

    #[test]
    fn test_zero_range_protection() {
        // Intentionally create invalid range -> expect panic in constructor
        let result = std::panic::catch_unwind(|| {
            Sensor::new(
                0,
                Point2 { x: 0.0, y: 0.0 },
                Vector2 { x: 1.0, y: 1.0 },
                4,
                4,
                1.0,
            )
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_out_of_bounds_update_silences() {
        let mut s = Sensor::new(
            0,
            Point2 { x: 0.0, y: 0.0 },
            Vector2 { x: 1.0, y: 1.0 },
            1,
            3,
            1.0,
        );

        // Provide too short spectrum
        let spectrum = vec![synth_bin(0.5)]; // only 1 bin
        s.update(&spectrum);
        assert_eq!(s.activation(), 0.0);
    }
}
