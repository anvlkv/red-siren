use fundsp::{Float, Real};

use super::Scheme;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum Phase<S: Real + Float> {
    #[default]
    /// phase 0. No activity; envelope is 0.
    Idle,
    /// phase 1. Move upward toward `target`
    Attack { steps: u64, target: S },
    /// phase 2. Move downward toward `target` (non-zero)
    Decay { steps: u64, target: S },
    /// phase 3. Keep current control value (no change)
    Sustain { steps: u64 },
    /// phase 4. Move downward toward 0.0
    Release { steps: u64 },
}

impl<S: Real + Float> Phase<S> {
    fn remaining_steps(&self) -> u64 {
        match *self {
            Self::Idle => 0,
            Self::Sustain { steps, .. }
            | Self::Attack { steps, .. }
            | Self::Decay { steps, .. }
            | Self::Release { steps, .. } => steps,
        }
    }

    fn tick(self) -> Option<Self> {
        match self {
            Self::Idle => Some(Self::Idle),
            Self::Attack { steps, target } => steps
                .checked_sub(1)
                .map(|s| Self::Attack { steps: s, target }),
            Self::Decay { steps, target } => steps
                .checked_sub(1)
                .map(|s| Self::Decay { steps: s, target }),
            Self::Sustain { steps } => steps.checked_sub(1).map(|s| Self::Sustain { steps: s }),
            Self::Release { steps } => steps.checked_sub(1).map(|s| Self::Release { steps: s }),
        }
    }

    fn update_sample_rate(&mut self, d: f64, scheme: &Scheme, old_scheme: &Scheme) {
        match self {
            Self::Idle => {}
            Self::Attack { steps, .. } => {
                let r = old_scheme.atack - *steps;
                *steps = scheme.atack - (r as f64 * d).ceil() as u64;
            }
            Self::Decay { steps, .. } => {
                let r = old_scheme.decay - *steps;
                *steps = scheme.decay - (r as f64 * d).ceil() as u64;
            }
            Self::Sustain { steps } => {
                let r = old_scheme.sustain - *steps;
                *steps = scheme.sustain - (r as f64 * d).ceil() as u64;
            }
            Self::Release { steps } => {
                let r = old_scheme.release - *steps;
                *steps = scheme.release - (r as f64 * d).ceil() as u64;
            }
        }
    }

    fn next_phase(self, scheme: &Scheme, shape: S) -> Self {
        match self {
            Self::Idle => Self::Idle,
            Self::Attack { target, .. } => Self::Decay {
                steps: scheme.decay,
                target: target * shape,
            },
            Self::Decay { .. } => Self::Sustain {
                steps: scheme.sustain,
            },
            Self::Sustain { .. } => Self::Release {
                steps: scheme.release,
            },
            Self::Release { .. } => Self::Idle,
        }
    }
}
