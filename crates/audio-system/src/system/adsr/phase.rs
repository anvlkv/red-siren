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
    /// Remaining steps in the current phase (0 = last step, about to complete).
    pub(super) fn remaining_steps(&self) -> u64 {
        match *self {
            Self::Idle => 0,
            Self::Sustain { steps, .. }
            | Self::Attack { steps, .. }
            | Self::Decay { steps, .. }
            | Self::Release { steps, .. } => steps,
        }
    }

    /// The amplitude target for the current phase.
    ///
    /// - `Attack`  → the attack peak target
    /// - `Decay`   → the sustain level target
    /// - `Sustain` → `running_value` (no change)
    /// - `Release` → `S::zero()`
    /// - `Idle`    → `S::zero()`
    pub(super) fn target_value(&self, running_value: S) -> S {
        match *self {
            Self::Idle => S::zero(),
            Self::Attack { target, .. } => target,
            Self::Decay { target, .. } => target,
            Self::Sustain { .. } => running_value,
            Self::Release { .. } => S::zero(),
        }
    }

    /// Decrement the step counter by one.
    ///
    /// Returns `Some(updated_phase)` while steps remain, `None` when this
    /// phase's last step has been consumed and the caller should transition via
    /// [`Self::next_phase`].
    pub(super) fn tick(self) -> Option<Self> {
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

    /// Rescale remaining steps after a sample-rate change.
    ///
    /// `d` is `new_sr / old_sr`.  `scheme` and `old_scheme` provide the full
    /// step counts at the new and old rates respectively.
    pub(super) fn update_sample_rate(&mut self, d: f64, scheme: &Scheme, old_scheme: &Scheme) {
        match self {
            Self::Idle => {}
            Self::Attack { steps, .. } => {
                let elapsed = old_scheme.atack.saturating_sub(*steps);
                let new_elapsed = (elapsed as f64 * d).ceil() as u64;
                *steps = scheme.atack.saturating_sub(new_elapsed);
            }
            Self::Decay { steps, .. } => {
                let elapsed = old_scheme.decay.saturating_sub(*steps);
                let new_elapsed = (elapsed as f64 * d).ceil() as u64;
                *steps = scheme.decay.saturating_sub(new_elapsed);
            }
            Self::Sustain { steps } => {
                let elapsed = old_scheme.sustain.saturating_sub(*steps);
                let new_elapsed = (elapsed as f64 * d).ceil() as u64;
                *steps = scheme.sustain.saturating_sub(new_elapsed);
            }
            Self::Release { steps } => {
                let elapsed = old_scheme.release.saturating_sub(*steps);
                let new_elapsed = (elapsed as f64 * d).ceil() as u64;
                *steps = scheme.release.saturating_sub(new_elapsed);
            }
        }
    }

    /// Advance to the next ADSR stage after the current one completes.
    ///
    /// `shape` is the decay-level multiplier applied to the attack peak:
    /// `decay_target = attack_target * shape`.
    pub(super) fn next_phase(self, scheme: &Scheme, shape: S) -> Self {
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
