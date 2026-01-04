use std::f64::consts::FRAC_1_PI;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Japanese Scale Interval Patterns
pub enum Scale {
    /// Yo Scale (bright)
    #[default]
    Yo,
    /// In Scale (dark)
    In,
}

impl Scale {
    pub(crate) fn freq_n(&self, nth: f64, base_freq: f64, total_steps: f64) -> f64 {
        match self {
            Scale::Yo => base_freq * 2_f64.powf(nth / total_steps),
            Scale::In => {
                let m = total_steps + total_steps * FRAC_1_PI;
                base_freq * ((2.0 * m) / (m + (total_steps - 1.0 - nth)))
            }
        }
    }

    pub fn is_dark(&self) -> bool {
        matches!(self, Scale::In)
    }
}
