use crate::util::{SComplex, S};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Adsr {
    value: S,
    secondary_value: S,
    shape: S,
    phase: AdsrPhase,
    scheme: Scheme,
    sample_rate: u64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
enum AdsrPhase {
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

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Scheme {
    atack: u64,
    decay: u64,
    sustain: u64,
    release: u64,
}

impl AdsrPhase {
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
            AdsrPhase::Idle => Some(AdsrPhase::Idle),
            AdsrPhase::Attack { steps, target } => steps
                .checked_sub(1)
                .map(|s| AdsrPhase::Attack { steps: s, target }),
            AdsrPhase::Decay { steps, target } => steps
                .checked_sub(1)
                .map(|s| AdsrPhase::Decay { steps: s, target }),
            AdsrPhase::Sustain { steps } => steps
                .checked_sub(1)
                .map(|s| AdsrPhase::Sustain { steps: s }),
            AdsrPhase::Release { steps } => steps
                .checked_sub(1)
                .map(|s| AdsrPhase::Release { steps: s }),
        }
    }

    fn update_sample_rate(&mut self, d: f64, scheme: &Scheme, old_scheme: &Scheme) {
        match self {
            AdsrPhase::Idle => {}
            AdsrPhase::Attack { steps, .. } => {
                let r = old_scheme.atack - *steps;
                *steps = scheme.atack - (r as f64 * d).ceil() as u64;
            }
            AdsrPhase::Decay { steps, .. } => {
                let r = old_scheme.decay - *steps;
                *steps = scheme.decay - (r as f64 * d).ceil() as u64;
            }
            AdsrPhase::Sustain { steps } => {
                let r = old_scheme.sustain - *steps;
                *steps = scheme.sustain - (r as f64 * d).ceil() as u64;
            }
            AdsrPhase::Release { steps } => {
                let r = old_scheme.release - *steps;
                *steps = scheme.release - (r as f64 * d).ceil() as u64;
            }
        }
    }

    fn next_phase(self, scheme: &Scheme, shape: S) -> Self {
        match self {
            AdsrPhase::Idle => AdsrPhase::Idle,
            AdsrPhase::Attack { target, .. } => AdsrPhase::Decay {
                steps: scheme.decay,
                target: target * shape,
            },
            AdsrPhase::Decay { .. } => AdsrPhase::Sustain {
                steps: scheme.sustain,
            },
            AdsrPhase::Sustain { .. } => AdsrPhase::Release {
                steps: scheme.release,
            },
            AdsrPhase::Release { .. } => AdsrPhase::Idle,
        }
    }
}

impl Adsr {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            value: 0.0,
            secondary_value: 0.0,
            shape: 0.0,
            phase: AdsrPhase::Idle,
            scheme: Scheme::default(),
            sample_rate: sample_rate as u64,
        }
    }

    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        let d = self.sample_rate as f64 / sample_rate;
        let old_scheme = self.scheme;
        self.sample_rate = sample_rate as u64;
        self.scheme.atack = (self.scheme.atack as f64 * d) as u64;
        self.scheme.decay = (self.scheme.decay as f64 * d) as u64;
        self.scheme.sustain = (self.scheme.sustain as f64 * d) as u64;
        self.scheme.release = (self.scheme.release as f64 * d) as u64;
        self.phase.update_sample_rate(d, &self.scheme, &old_scheme);
    }

    pub fn update(&mut self, prev: SComplex, next: SComplex) {
        self.secondary_value = next.im;

        // Normalize and compute dynamics
        let target = next.re;
        let delta = target - prev.re;
        let delta_im = next.im - prev.im;

        let current_value = self.value;
        let current_target = self.target_value();
        let current_delta = (current_target - current_value).clamp(-1.0, 1.0);

        // Meta: "new demand" minus "current debt"
        let meta = (delta - current_delta).clamp(-1.0, 1.0);

        // Base distances for each potential motion
        let distance_up = (target - current_value).max(0.0);
        let distance_down = (current_value - target).max(0.0);
        let distance_to_zero = current_value;

        // Weight distances by meta pressure:
        // - positive meta emphasizes upward motion
        // - negative meta emphasizes downward motion
        // - near zero meta emphasizes holding
        //
        // We use sample_rate directly to make the steps sensitive to how often we tick.
        // Larger weighted distance → more steps → smaller per-tick increment (smoother movement).
        let sr = self.sample_rate as S;

        // Helper: compute steps from a weighted distance (no fixed constants).
        let steps_from = |weighted_distance: S| -> u64 {
            if weighted_distance <= S::EPSILON {
                0
            } else {
                // Scale by sample rate to keep behavior consistent with time.
                (weighted_distance * sr * (1.0 - delta_im)).ceil() as u64
            }
        };

        // Shape is the factor applied to the Decay target on phase transition.
        // Use meta as the dynamic shaper:
        // - positive meta: keep more of the attack (higher sustain proportion)
        // - negative meta: reduce sustain proportion
        // - clamp into [0,1]
        self.shape = (0.5 + meta * delta_im).abs().clamp(0.0, 1.0);

        // Proposed scheme derived purely from current distances and meta (no consts).
        // Note: we avoid division-by-zero by letting steps be zero when distances are zero.
        let proposed_attack_steps = steps_from(distance_up * meta.max(0.0));
        let proposed_decay_steps = steps_from(distance_down * (-meta).max(0.0));
        let proposed_release_steps = steps_from(distance_to_zero * (-meta).max(0.0));

        // Sustain: if target ~ current_value and meta ~ 0, hold. Otherwise, sustain budget is zero.
        let proposed_sustain_steps =
            if meta.abs() < S::EPSILON && (current_target - current_value).abs() < S::EPSILON {
                // Hold is intentional when no pressure and no debt
                u64::MAX
            } else {
                0
            };

        // Update scheme
        let old_scheme = self.scheme;
        self.scheme = Scheme {
            atack: proposed_attack_steps,
            decay: proposed_decay_steps,
            sustain: proposed_sustain_steps,
            release: proposed_release_steps,
        };

        self.phase = if target <= S::EPSILON {
            if current_value <= S::EPSILON {
                AdsrPhase::Idle
            } else {
                // If already releasing, carry proportional remaining steps
                match self.phase {
                    AdsrPhase::Release { steps } if old_scheme.release > 0 => {
                        let progressed = old_scheme.release.saturating_sub(steps);
                        let r = if old_scheme.release == 0 {
                            0.0
                        } else {
                            progressed as f64 / old_scheme.release as f64
                        };
                        let new_remaining = (self.scheme.release as f64 * (1.0 - r)).ceil() as u64;
                        AdsrPhase::Release {
                            steps: new_remaining,
                        }
                    }
                    _ => AdsrPhase::Release {
                        steps: self.scheme.release,
                    },
                }
            }
        } else if meta > S::EPSILON {
            // Upward pressure: Attack
            match self.phase {
                AdsrPhase::Attack { steps, .. } if old_scheme.atack > 0 => {
                    let progressed = old_scheme.atack.saturating_sub(steps);
                    let r = if old_scheme.atack == 0 {
                        0.0
                    } else {
                        progressed as f64 / old_scheme.atack as f64
                    };
                    let new_remaining = (self.scheme.atack as f64 * (1.0 - r)).ceil() as u64;
                    AdsrPhase::Attack {
                        steps: new_remaining,
                        target,
                    }
                }
                // If we were decaying but meta flipped positive, revector to Attack
                _ => AdsrPhase::Attack {
                    steps: self.scheme.atack,
                    target,
                },
            }
        } else if meta < -S::EPSILON {
            // Downward pressure: Decay (toward new target), or Release if target ~ 0
            match self.phase {
                AdsrPhase::Decay { steps, .. } if old_scheme.decay > 0 => {
                    let progressed = old_scheme.decay.saturating_sub(steps);
                    let r = if old_scheme.decay == 0 {
                        0.0
                    } else {
                        progressed as f64 / old_scheme.decay as f64
                    };
                    let new_remaining = (self.scheme.decay as f64 * (1.0 - r)).ceil() as u64;
                    AdsrPhase::Decay {
                        steps: new_remaining,
                        target,
                    }
                }
                AdsrPhase::Attack { .. } => {
                    // Flip directly to Decay if pressure demands it
                    AdsrPhase::Decay {
                        steps: self.scheme.decay,
                        target,
                    }
                }
                _ => {
                    // If current target is approx 0, prefer Release
                    if target <= S::EPSILON {
                        AdsrPhase::Release {
                            steps: self.scheme.release,
                        }
                    } else {
                        AdsrPhase::Decay {
                            steps: self.scheme.decay,
                            target,
                        }
                    }
                }
            }
        } else {
            // No pressure: Sustain if aligned, otherwise keep phase with a minimal step budget.
            if (current_target - current_value).abs() <= S::EPSILON {
                AdsrPhase::Sustain {
                    steps: self.scheme.sustain,
                }
            } else {
                // Preserve current phase but with recalculated steps for small corrections
                match self.phase {
                    AdsrPhase::Attack { .. } => AdsrPhase::Attack {
                        steps: self.scheme.atack,
                        target,
                    },
                    AdsrPhase::Decay { .. } => AdsrPhase::Decay {
                        steps: self.scheme.decay,
                        target,
                    },
                    AdsrPhase::Sustain { .. } => AdsrPhase::Sustain {
                        steps: self.scheme.sustain,
                    },
                    AdsrPhase::Release { .. } => AdsrPhase::Release {
                        steps: self.scheme.release,
                    },
                    AdsrPhase::Idle => {
                        // If idle but not aligned, choose direction purely by sign of (target - value)
                        if target > current_value {
                            AdsrPhase::Attack {
                                steps: self.scheme.atack,
                                target,
                            }
                        } else if target < current_value {
                            AdsrPhase::Decay {
                                steps: self.scheme.decay,
                                target,
                            }
                        } else {
                            AdsrPhase::Idle
                        }
                    }
                }
            }
        };
    }

    pub fn tick(&mut self) -> (S, S) {
        let target = self.target_value();
        let steps = self.phase.remaining_steps();
        let increment = if steps > 0 {
            (target - self.value) / (steps as S)
        } else {
            0.0
        };

        let primary = if let Some(next_phase) = self.phase.tick() {
            self.phase = next_phase;
            self.value += increment;
            self.value
        } else {
            self.phase = self.phase.next_phase(&self.scheme, self.shape);
            self.value = target;
            self.value
        };

        let secondary = self.secondary_value * primary;

        (primary, secondary)
    }

    fn target_value(&self) -> S {
        match self.phase {
            AdsrPhase::Idle | AdsrPhase::Release { .. } => 0.0,
            AdsrPhase::Sustain { .. } => self.value,
            AdsrPhase::Attack { target, .. } => target,
            AdsrPhase::Decay { target, .. } => target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;
    const EPS: S = 1.0e-6;

    #[test]
    fn tick_idle_returns_zero() {
        let mut env = Adsr::new(SR);
        let (v, _) = env.tick();
        assert!(
            (v - 0.0).abs() <= EPS,
            "Idle tick should return zero, got {}",
            v
        );
    }

    #[test]
    fn attack_reaches_target_one() {
        let mut env = Adsr::new(SR);
        // Demand an upward move to 1.0
        env.update(SComplex::ZERO, SComplex::ONE);

        // Attack steps are approximately sample_rate when meta=1 and distance_up=1.
        // Tick slightly more than SR to ensure completion and phase transition.
        let mut has_reached_one = false;
        for _ in 0..(SR as usize + 50) {
            if (env.tick().0 - 1.0).abs() <= EPS {
                has_reached_one = true;
            }
        }
        assert!(has_reached_one, "Envelope should reach ~1.0 after attack",);
    }

    #[test]
    fn release_reaches_zero() {
        let mut env = Adsr::new(SR);
        // Go up first
        env.update(SComplex::ZERO, SComplex::ONE);
        let mut v = 0.0;
        for _ in 0..(SR as usize + 10) {
            v = env.tick().0;
        }
        // Now request release to zero
        env.update(SComplex::new(v, v), SComplex::ZERO);
        for _ in 0..(SR as usize + 10) {
            v = env.tick().0;
        }
        assert!(v <= EPS, "Envelope should release to ~0.0, got {}", v);
    }
}
