use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Japanese Scale Interval Patterns
pub enum Scale {
    /// Yo Scale (bright)
    ///
    /// [wikipedia](https://en.wikipedia.org/wiki/Yo_scale)
    ///
    /// Intervals: ascending intervals of `whole, minor third, whole, whole, minor third` semitones
    #[default]
    Yo,
    /// In Scale (dark)
    ///
    /// [wikipedia](https://en.wikipedia.org/wiki/In_scale)
    ///
    /// Intervals: ascending intervals of `semitone, major third, whole, semitone, major third` semitones
    In,
}

impl Scale {
    fn tet_steps(&self) -> [u8; 5] {
        match self {
            Self::Yo => [0, 2, 5, 7, 9],
            Self::In => [0, 1, 5, 7, 8],
        }
    }

    fn step(&self, n_divisions: u8, k_step: u8) -> u8 {
        let tet_steps = self.tet_steps();
        let num_steps = tet_steps.len() as u8;
        let step_index = k_step % num_steps;
        let octave_offset = (k_step / num_steps) * n_divisions;
        octave_offset + tet_steps[step_index as usize]
    }

    pub fn k_frequency(&self, n_divisions: u8, k_step: u8, base_frequency: f64) -> f64 {
        let step = self.step(n_divisions, k_step);
        base_frequency * 2f64.powf(step as f64 / n_divisions as f64)
    }
}
