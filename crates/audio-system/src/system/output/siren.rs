#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::{hash_str, S};

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));

/// Siren oscillator with excitement-controlled pauses and frequency.
/// - Input 0: excitement level
/// - Input 1: base_hz
/// - Input 2: max_frequency_hz
/// - Input 3: excitement_pause_limit
/// - Input 4: base_pause_duration
/// - Output 0: siren wave with pauses and frequency.
#[derive(Default, Clone)]
pub struct Siren<F: Real> {
    phase: F,
    sample_duration: F,
    hash: u64,
}

impl<F: Real> Siren<F> {
    /// Create siren oscillator.
    pub fn new() -> Self {
        let mut siren = Siren::default();
        siren.reset();
        siren.set_sample_rate(DEFAULT_SR);
        siren
    }

    fn tick_internal(
        &self,
        excitement: S,
        siren_base_hz: S,
        max_frequency_hz: S,
        excitement_pause_limit: S,
        base_pause_duration: S,
    ) -> (S, F) {
        if excitement <= 0.0 {
            return (S::zero(), F::zero());
        }

        let interpolated_frequency =
            siren_base_hz + excitement * (max_frequency_hz - siren_base_hz);

        #[cfg(feature = "hi_fi")]
        let phase = self.phase.to_f64();

        #[cfg(not(feature = "hi_fi"))]
        let phase = self.phase.to_f32();

        #[cfg(feature = "hi_fi")]
        let sample_duration = self.sample_duration.to_f64();

        #[cfg(not(feature = "hi_fi"))]
        let sample_duration = self.sample_duration.to_f32();

        let previous_sin_abs = sin(phase * S::TAU).abs();
        let projected_sin_abs =
            sin((phase + interpolated_frequency * sample_duration) * S::TAU).abs();

        let gaining = previous_sin_abs < projected_sin_abs;

        let phase_increment = if previous_sin_abs > excitement_pause_limit
            || gaining
            || excitement > excitement_pause_limit
        {
            interpolated_frequency * sample_duration
        } else {
            let slow = (base_pause_duration / sample_duration) * (1.0 - excitement);
            (interpolated_frequency * sample_duration) / slow
        };

        let sample = sin(phase * S::TAU);

        #[cfg(feature = "hi_fi")]
        let phase_increment = F::from_f64(phase_increment);

        #[cfg(not(feature = "hi_fi"))]
        let phase_increment = F::from_f32(phase_increment);

        (sample, phase_increment)
    }
}

impl<F: Real> AudioNode for Siren<F> {
    const ID: u64 = SIREN_ID;
    type Inputs = typenum::U5;
    type Outputs = typenum::U1;

    fn reset(&mut self) {
        self.phase = F::zero();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_duration = convert(1.0 / sample_rate);
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let excitement = input[0] as S;

        // Get fine-tuned values from inputs
        let siren_base_hz = input[1] as S;
        let max_frequency_hz = input[2] as S;
        let excitement_pause_limit = input[3] as S;
        let base_pause_duration = input[4] as S;

        let (sample, phase_increment) = self.tick_internal(
            excitement,
            siren_base_hz,
            max_frequency_hz,
            excitement_pause_limit,
            base_pause_duration,
        );

        self.phase += phase_increment;
        self.phase -= self.phase.floor();

        [sample as f32].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {}

    fn set_hash(&mut self, hash: u64) {
        self.hash = hash;
        self.reset();
    }
}

pub fn siren<F>() -> An<Siren<F>>
where
    F: Real,
{
    let siren = Siren::new();
    An(siren)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_FREQ: f32 = 0.5;
    const MAX_FREQ: f32 = 775.0;
    const PAUSE_LIMIT: f32 = 0.2;
    const PAUSE_DURATION: f32 = 0.7;

    #[test]
    fn test_siren_behavior() {
        // Test with zero input
        let mut siren_node = siren::<f32>();
        siren_node.reset();

        let mut outputs = Vec::new();

        // Process 2 seconds worth of samples with zero input
        for _ in 0..96000 {
            let input: Frame<f32, typenum::U5> =
                [0.0, MIN_FREQ, MAX_FREQ, PAUSE_LIMIT, PAUSE_DURATION].into();
            let output = siren_node.tick(&input);
            outputs.push(output[0]);
        }

        // All outputs should be zero
        assert!(
            outputs.iter().all(|&x| x == 0.0),
            "Should be silent when input is 0"
        );

        // Test with positive input (should oscillate)
        outputs.clear();

        for _ in 0..96000 {
            let input: Frame<f32, typenum::U5> =
                [0.3, MIN_FREQ, MAX_FREQ, PAUSE_LIMIT, PAUSE_DURATION].into();
            let output = siren_node.tick(&input);
            outputs.push(output[0]);
        }

        let has_positive = outputs.iter().any(|&x| x > 0.01);
        let has_negative = outputs.iter().any(|&x| x < -0.01);

        assert!(has_positive, "Should show positive values when input > 0");

        assert!(has_negative, "Should show negative values when input > 0");
    }

    #[test]
    fn test_frequency_interpolation() {
        let mut siren_node = siren::<f32>();
        siren_node.reset();

        // Test that frequency is interpolated based on input amplitude
        // Test with a=0.5, frequency should be in the middle
        let mut sign_changes = 0;
        let mut previous_output = 0.0;

        // Run for 2 seconds at 48kHz to account for initial low frequency
        for _ in 0..96000 {
            let input: Frame<f32, typenum::U5> =
                [0.3, MIN_FREQ, MAX_FREQ, PAUSE_LIMIT, PAUSE_DURATION].into(); // Mid excitement level
            let output = siren_node.tick(&input);

            // Detect zero crossings (positive to negative)
            if previous_output > 0.0 && output[0] <= 0.0 {
                sign_changes += 1;
            }
            previous_output = output[0];
        }

        assert!(
            (15..=45).contains(&sign_changes),
            "With a=0.3 and pauses, expected 15-45 sign changes in 2 seconds, got {}",
            sign_changes
        );

        // Test with different input levels
        siren_node.reset();
        sign_changes = 0;
        previous_output = 0.0;

        // Test with very low excitement (a=0.1) for 2 seconds
        for _ in 0..96000 {
            let input: Frame<f32, typenum::U5> =
                [0.1, MIN_FREQ, MAX_FREQ, PAUSE_LIMIT, PAUSE_DURATION].into();
            let output = siren_node.tick(&input);

            if previous_output > 0.0 && output[0] <= 0.0 {
                sign_changes += 1;
            }
            previous_output = output[0];
        }

        assert!(
            (8..=25).contains(&sign_changes),
            "With a=0.1 and longer pauses, expected 8-25 sign changes in 2 seconds, got {}",
            sign_changes
        );
    }
}
