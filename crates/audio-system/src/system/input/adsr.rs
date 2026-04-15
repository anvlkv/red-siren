use fundsp::{Float, Real};
use num_complex::Complex;

/// Attack time constant (seconds).
const ATTACK_S: f64 = 0.030;
/// Release time constant (seconds).
const RELEASE_S: f64 = 0.500;

/// Simple per-node ADSR envelope smoother used by the FFT analyzer.
///
/// Smooths raw FFT-derived excitation values before they are written to the
/// [`crate::system::excitor::control::Control`] shared values. Independent
/// attack and release coefficients are derived from the sample rate so that
/// onsets are captured quickly while tails decay naturally.
///
/// ## Usage in the audio graph
///
/// One `Adsr<S>` is kept per [`common::NodeKey`] inside [`super::analyzer::FFTAnalyzer`]:
///
/// - `tick()` is called on every audio sample to advance the envelope and
///   write the smoothed value to the control.
/// - `update(prev, current)` is called whenever the FFT analysis thread
///   produces a new excitation estimate for this key.
#[derive(Clone)]
pub struct Adsr<S: Real + Float> {
    /// Currently emitted value – real component.
    current_re: S,
    /// Currently emitted value – imaginary component.
    current_im: S,
    /// Target toward which the envelope is moving – real component.
    target_re: S,
    /// Target toward which the envelope is moving – imaginary component.
    target_im: S,
    /// Per-sample smoothing coefficient for a *rising* envelope (attack).
    alpha_attack: S,
    /// Per-sample smoothing coefficient for a *falling* envelope (release).
    alpha_release: S,
}

impl<S: Real + Float> Adsr<S> {
    /// Create an envelope smoother calibrated for `sample_rate` Hz.
    pub fn new(sample_rate: f64) -> Self {
        let alpha_a = Self::compute_alpha(ATTACK_S, sample_rate);
        let alpha_r = Self::compute_alpha(RELEASE_S, sample_rate);
        Self {
            current_re: S::zero(),
            current_im: S::zero(),
            target_re: S::zero(),
            target_im: S::zero(),
            alpha_attack: S::from_f64(alpha_a),
            alpha_release: S::from_f64(alpha_r),
        }
    }

    /// First-order IIR coefficient for a given time constant τ (seconds).
    ///
    /// α = 1 − exp(−1 / (τ · fs))
    fn compute_alpha(tau_s: f64, sample_rate: f64) -> f64 {
        if sample_rate <= 0.0 || tau_s <= 0.0 {
            return 1.0; // instantaneous follow
        }
        1.0 - (-1.0_f64 / (tau_s * sample_rate)).exp()
    }

    /// Advance the envelope by one sample and return the current `(re, im)` pair.
    ///
    /// The return type matches the argument expected by
    /// [`crate::system::excitor::control::Control::set_value`].
    pub fn tick(&mut self) -> (S, S) {
        // Choose attack or release coefficient independently per component.
        let alpha_re = if self.target_re > self.current_re {
            self.alpha_attack
        } else {
            self.alpha_release
        };
        let alpha_im = if self.target_im > self.current_im {
            self.alpha_attack
        } else {
            self.alpha_release
        };

        self.current_re = self.current_re + alpha_re * (self.target_re - self.current_re);
        self.current_im = self.current_im + alpha_im * (self.target_im - self.current_im);

        (self.current_re, self.current_im)
    }

    /// Inform the smoother of a new FFT-derived excitation target.
    ///
    /// `_prev` is the previous complex excitation value; it is accepted for
    /// API symmetry with the call-site in `tick_adsr` but unused — the
    /// smoother already tracks `current_re`/`current_im` internally.
    pub fn update(&mut self, _prev: Complex<S>, current: Complex<S>) {
        self.target_re = current.re;
        self.target_im = current.im;
    }

    /// Recompute the smoothing coefficients when the audio pipeline sample
    /// rate changes (e.g. after a device switch).
    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        self.alpha_attack = S::from_f64(Self::compute_alpha(ATTACK_S, sample_rate));
        self.alpha_release = S::from_f64(Self::compute_alpha(RELEASE_S, sample_rate));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let adsr = Adsr::<f32>::new(44100.0);
        let mut a = adsr.clone();
        let (re, im) = a.tick();
        assert!(re.abs() < 1e-9, "re should start near zero");
        assert!(im.abs() < 1e-9, "im should start near zero");
    }

    #[test]
    fn tick_approaches_target() {
        let mut adsr = Adsr::<f32>::new(44100.0);
        adsr.update(Complex::new(0.0, 0.0), Complex::new(1.0, 0.5));

        // Run for 100 ms worth of samples.
        let samples = (44100.0 * 0.1) as usize;
        let mut last = (0.0_f32, 0.0_f32);
        for _ in 0..samples {
            last = adsr.tick();
        }
        // Should have made meaningful progress toward target within 100 ms.
        assert!(
            last.0 > 0.1,
            "re should progress toward 1.0, got {}",
            last.0
        );
        assert!(
            last.1 > 0.05,
            "im should progress toward 0.5, got {}",
            last.1
        );
    }

    #[test]
    fn release_is_slower_than_attack() {
        let mut adsr = Adsr::<f32>::new(44100.0);
        // Fully charge the envelope.
        adsr.update(Complex::new(0.0, 0.0), Complex::new(1.0, 0.0));
        for _ in 0..(44100 * 2) {
            adsr.tick();
        }

        // Now release to zero.
        adsr.update(Complex::new(1.0, 0.0), Complex::new(0.0, 0.0));
        let samples_10ms = (44100.0 * 0.01) as usize;
        let mut after_10ms = 0.0_f32;
        for _ in 0..samples_10ms {
            (after_10ms, _) = adsr.tick();
        }
        // After 10 ms the release should still be close to 1.0 (slow tail).
        assert!(
            after_10ms > 0.9,
            "release should still be high after 10 ms, got {after_10ms}"
        );
    }

    #[test]
    fn zero_sample_rate_does_not_panic() {
        let mut adsr = Adsr::<f32>::new(0.0);
        adsr.update(Complex::new(0.0, 0.0), Complex::new(0.5, 0.25));
        let (re, _im) = adsr.tick();
        assert!(re.is_finite());
    }
}
