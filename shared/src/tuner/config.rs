/*
Tuner input gain calibration

Given sparse calibration points (frequency -> dB offset), provides:
- gain_db(f): interpolated dB gain
- gain_linear(f): linear amplitude multiplier

Interpolation is piecewise-linear in log-frequency space (perceptually smoother).

Calibration table (Hz -> dB):
20      -> -2.7
100     -> +0.7
1000    -> +0.5
10000   -> +3.5

Frequencies outside table range are clamped to nearest endpoint.
*/

use crate::instrument::consts::{MAX_FREQ_HZ, MIN_FREQ_HZ};

/// A single calibration point (frequency in Hz, gain in dB)
#[derive(Debug, Clone, Copy)]
pub struct CalPoint {
    pub freq_hz: f64,
    pub gain_db: f32,
}

/// Static calibration curve (sorted by ascending freq)
const CAL_POINTS: &[CalPoint] = &[
    CalPoint {
        freq_hz: 20.0,
        gain_db: -2.7,
    },
    CalPoint {
        freq_hz: 100.0,
        gain_db: 0.7,
    },
    CalPoint {
        freq_hz: 1000.0,
        gain_db: 0.5,
    },
    CalPoint {
        freq_hz: 10_000.0,
        gain_db: 3.5,
    },
];

/// Return interpolated gain in dB for a given frequency (Hz).
///
/// Method:
/// - Clamp frequency to [table_min, table_max].
/// - If exactly matches a point, return its dB.
/// - Otherwise, find the two surrounding points (p0, p1).
/// - Interpolate linearly in log10(frequency) space.
pub fn gain_db(freq_hz: f64) -> f32 {
    // Clamp to calibration domain (independent of absolute MIN/MAX, but we also
    // guard against non-positive frequencies)
    let f = freq_hz.clamp(MIN_FREQ_HZ.max(1.0), MAX_FREQ_HZ).clamp(
        CAL_POINTS.first().unwrap().freq_hz,
        CAL_POINTS.last().unwrap().freq_hz,
    );

    // Exact match scan (tiny table => linear scan fine)
    for p in CAL_POINTS {
        if (p.freq_hz - f).abs() < f * 1e-12 {
            return p.gain_db;
        }
    }

    // Find enclosing segment
    let mut left = CAL_POINTS[0];
    let mut right = CAL_POINTS[CAL_POINTS.len() - 1];
    for w in CAL_POINTS.windows(2) {
        let a = w[0];
        let b = w[1];
        if f >= a.freq_hz && f <= b.freq_hz {
            left = a;
            right = b;
            break;
        }
    }

    // If collapsed (shouldn't happen), return left
    if (right.freq_hz - left.freq_hz).abs() <= f64::EPSILON {
        return left.gain_db;
    }

    let log_f = f.log10();
    let log_l = left.freq_hz.log10();
    let log_r = right.freq_hz.log10();
    let t = ((log_f - log_l) / (log_r - log_l)).clamp(0.0, 1.0);

    left.gain_db + (right.gain_db - left.gain_db) * t as f32
}

/// Return linear amplitude multiplier for a given frequency (Hz).
///
/// Conversion: linear = 10^(dB / 20)
pub fn gain_linear(freq_hz: f64) -> f32 {
    let db = gain_db(freq_hz);
    (10f32).powf(db / 20.0)
}

/// Expose calibration points if external inspection is desired.
pub fn calibration_points() -> &'static [CalPoint] {
    CAL_POINTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gain_db_endpoints() {
        for p in calibration_points() {
            let got = gain_db(p.freq_hz);
            assert!(
                (got - p.gain_db).abs() < 1e-6,
                "endpoint freq {} expected {} dB got {} dB",
                p.freq_hz,
                p.gain_db,
                got
            );
        }
    }

    #[test]
    fn test_gain_linear_consistency() {
        for &f in &[20.0, 100.0, 1000.0, 10_000.0] {
            let db = gain_db(f);
            let lin = gain_linear(f);
            let expected = (10f32).powf(db / 20.0);
            assert!(
                (lin - expected).abs() < 1e-6,
                "linear mismatch freq {} expected {} got {}",
                f,
                expected,
                lin
            );
        }
    }
}
