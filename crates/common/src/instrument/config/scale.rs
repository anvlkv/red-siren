use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Japanese Scale Interval Patterns
pub enum Scale {
    /// Yo Scale (bright pentatonic)
    ///
    /// Semitone sequence: 2 - 3 - 2 - 2 - 3
    ///
    /// Formula (counted from tonic, C): C, D (+2), F (+5), G (+7), A (+9)
    #[default]
    Yo,
    /// In Scale (dark pentatonic)
    ///
    /// Semitone sequence: 1 - 4 - 1 - 4 - 2
    ///
    /// Formula: C, D♭ (+1), F (+5), G (+7), A♭ (+8)
    In,
}

impl Scale {
    pub(crate) fn freq_n(&self, nth: f64, base_freq: f64, total_steps: f64) -> f64 {
        match self {
            Scale::Yo => base_freq * 2_f64.powf(nth / total_steps),
            Scale::In => (2_f64 * base_freq) / 2_f64.powf(nth / total_steps),
        }
    }
}
