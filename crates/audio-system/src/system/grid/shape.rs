use fundsp::prelude::*;

/// ADSR shape parameters expressed as fractions of the total note duration.
///
/// `attack` and `release` are ratios in `[0.0, 1.0]` of the total duration.
/// `decay` is a non-negative ratio of the total duration and may exceed `1.0`
/// to intentionally produce very long decay phases.
/// `sustain` is an amplitude level in `[0.0, 1.0]`.
#[derive(Clone)]
pub struct AdsrShape<S: Real + Float + 'static> {
    /// Fraction of total duration spent in attack (0 → 1). Default: 0.05
    pub attack: S,
    /// Fraction of total duration spent in decay (1 → sustain). Default: 0.10
    pub decay: S,
    /// Sustain amplitude level. Default: 0.7
    pub sustain: S,
    /// Fraction of total duration spent in release (sustain → 0). Default: 0.20
    pub release: S,
    /// Blend factor for rounded stage curves in [0.0, 1.0].
    /// 0.0 = linear ADSR segments, 1.0 = fully smoothed (smoothstep).
    pub smoothness: S,
    /// Decay-release tail power bend.
    ///
    /// `1.0` is neutral.
    /// Values `< 1.0` bias early tail drop (faster initial drop, longer tail).
    /// Values `> 1.0` bias late tail drop (slower initial drop, steeper tail).
    pub decay_exponent: S,
}

impl<S: Real + Float + 'static> Default for AdsrShape<S> {
    fn default() -> Self {
        Self {
            attack: S::from_f32(0.05),
            decay: S::from_f32(0.10),
            sustain: S::from_f32(0.7),
            release: S::from_f32(0.20),
            smoothness: S::from_f32(0.0),
            decay_exponent: S::from_f32(1.0),
        }
    }
}

impl<S: Real + Float + 'static> AdsrShape<S> {
    fn clamp_unit(value: S) -> S {
        let zero = S::zero();
        let one = S::one();
        if value < zero {
            zero
        } else if value > one {
            one
        } else {
            value
        }
    }

    fn sanitize_positive(value: S, fallback: S) -> S {
        if !value.to_f64().is_finite() || value <= S::zero() {
            fallback
        } else {
            value
        }
    }

    pub fn smooth_progress(t: S, smoothness: S) -> S {
        let t = Self::clamp_unit(t);
        let s = Self::clamp_unit(smoothness);
        let two = S::from_f64(2.0);
        let three = S::from_f64(3.0);

        let smoothstep = t * t * (three - two * t);
        t + (smoothstep - t) * s
    }

    pub fn tail_progress(t: S, smoothness: S, decay_exponent: S) -> S {
        let eased = Self::smooth_progress(t, smoothness);
        let exponent = Self::sanitize_positive(decay_exponent, S::one());
        // Inverted power curve: 1 - (1 - eased)^exponent
        // exponent < 1: slower approach (lingers near target)
        // exponent > 1: faster approach (quick transition)
        let inverted = 1.0 - eased.to_f64();
        let bent = 1.0 - inverted.powf(exponent.to_f64());
        Self::clamp_unit(S::from_f64(bent))
    }
}
