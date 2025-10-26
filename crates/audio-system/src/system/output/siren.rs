#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));

/// Siren oscillator with excitement-controlled pauses and frequency.
/// - Input 0: excitement level (a). Zero = silent, positive = oscillate with pauses.
/// - Input 1: siren_base_hz
/// - Input 2: siren_max_frequency_hz
/// - Input 3: siren_excitement_pause_limit
/// - Input 4: siren_base_pause_duration
/// - Output 0: siren wave with pauses and frequency.
#[derive(Default, Clone)]
pub struct Siren<F: Real> {
    freq: F,
    phase: F,
    sample_duration: F,
    pause_timer: F,   // Tracks remaining pause time
    previous_sine: F, // For zero-crossing detection
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

    /// Detects if we've crossed zero between previous and current sine values
    fn has_zero_crossed(&self, current_sine: F) -> bool {
        // Zero crossing occurs when signs differ and we're going from positive to negative
        self.previous_sine > F::zero() && current_sine <= F::zero()
    }

    /// Calculates pause duration based on input amplitude
    /// Higher amplitude = shorter pause, returns positive duration or zero
    fn calculate_pause_duration(
        &self,
        a: F,
        excitement_pause_limit: F,
        base_pause_duration: F,
    ) -> F {
        if (F::zero()..=excitement_pause_limit).contains(&a) {
            // Higher amplitude = shorter pause
            let pause_factor = (excitement_pause_limit - a).max(F::zero());
            base_pause_duration * pause_factor
        } else {
            F::zero()
        }
    }
}

impl<F: Real> AudioNode for Siren<F> {
    const ID: u64 = SIREN_ID;
    type Inputs = typenum::U5;
    type Outputs = typenum::U1;

    fn reset(&mut self) {
        self.phase = F::zero();
        self.pause_timer = F::zero();
        self.previous_sine = F::zero();
        self.freq = F::zero();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_duration = convert(1.0 / sample_rate);
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let a = F::from_f32(input[0]);

        // Get fine-tuned values from inputs
        let siren_base_hz = F::from_f32(input[1]);
        let max_frequency_hz = F::from_f32(input[2]);
        let excitement_pause_limit = F::from_f32(input[3]);
        let base_pause_duration = F::from_f32(input[4]);

        // Silent when input is zero
        if a <= F::zero() {
            self.pause_timer = F::zero();
            self.previous_sine = F::zero();
            self.freq = siren_base_hz;
            return [0.0].into();
        }

        // Handle pause state
        if self.pause_timer > F::zero() {
            // Still pausing
            self.pause_timer -= self.sample_duration;
            return [0.0].into();
        }

        // Generate sine wave (phase is in [0,1) range)
        let current_sine = sin(self.phase.to_f32() * f32::TAU);
        let current_sine_f = F::from_f32(current_sine);

        // Check for zero crossing (positive to negative)
        if self.has_zero_crossed(current_sine_f) {
            // Update frequency by interpolating based on input amplitude
            // a = 0 -> siren_base_hz, a = 1 -> max_frequency_hz
            self.freq = siren_base_hz + ((max_frequency_hz - siren_base_hz) * a);

            // Calculate and set pause duration
            self.pause_timer =
                self.calculate_pause_duration(a, excitement_pause_limit, base_pause_duration);

            // If there's a pause, start pausing now
            if self.pause_timer > F::zero() {
                self.previous_sine = current_sine_f;
                return [0.0].into();
            }
        }

        // Advance phase (frequency is in Hz, so multiply by sample duration)
        self.phase += self.freq * self.sample_duration;
        self.phase -= self.phase.floor();

        // Store current sine for next zero-crossing check
        self.previous_sine = current_sine_f;

        [current_sine].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        let mut phase = self.phase;
        let mut freq = self.freq;
        let mut pause_timer = self.pause_timer;
        let mut previous_sine = self.previous_sine;

        // Get fine-tuned values once per batch
        let siren_base_hz = F::from_f32(input.at_f32(1, 0));
        let max_frequency_hz = F::from_f32(input.at_f32(2, 0));
        let excitement_pause_limit = F::from_f32(input.at_f32(3, 0));
        let base_pause_duration = F::from_f32(input.at_f32(4, 0));

        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let a = F::from_f32(input.at_f32(0, (i << SIMD_S) + j));

                // Silent when input is zero
                if a <= F::zero() {
                    pause_timer = F::zero();
                    previous_sine = F::zero();
                    freq = siren_base_hz;
                    return 0.0;
                }

                // Handle pause state
                if pause_timer > F::zero() {
                    pause_timer -= self.sample_duration;
                    return 0.0;
                }

                // Generate sine wave (phase is in [0,1) range)
                let current_sine = sin(phase.to_f32() * f32::TAU);
                let current_sine_f = F::from_f32(current_sine);

                // Check for zero crossing (positive to negative)
                if previous_sine > F::zero() && current_sine_f <= F::zero() {
                    // Update frequency by interpolating based on input amplitude
                    // a = 0 -> siren_base_hz, a = 1 -> max_frequency_hz
                    freq = siren_base_hz + ((max_frequency_hz - siren_base_hz) * a);

                    // Calculate pause duration
                    pause_timer = if (F::zero()..=excitement_pause_limit).contains(&a) {
                        let pause_factor = (excitement_pause_limit - a).max(F::zero());
                        base_pause_duration * pause_factor
                    } else {
                        F::zero()
                    };

                    // If there's a pause, start pausing now
                    if pause_timer > F::zero() {
                        previous_sine = current_sine_f;
                        return 0.0;
                    }
                }

                // Advance phase (frequency is in Hz, so multiply by sample duration)
                phase += freq * self.sample_duration;
                phase -= phase.floor();

                // Store current sine for next zero-crossing check
                previous_sine = current_sine_f;

                current_sine
            });
            output.set(0, i, F32x::new(element));
        }

        self.phase = phase;
        self.freq = freq;
        self.pause_timer = pause_timer;
        self.previous_sine = previous_sine;
        self.process_remainder(size, input, output);
    }

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

    #[test]
    fn test_siren_behavior() {
        // Test with zero input
        let mut siren_node = siren::<f32>();
        siren_node.reset();

        let mut outputs = Vec::new();

        // Process 2 seconds worth of samples with zero input
        for _ in 0..96000 {
            let input: Frame<f32, typenum::U5> = [0.0, 0.5, 775.0, 0.4, 0.25].into();
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
            let input: Frame<f32, typenum::U5> = [0.3, 0.5, 775.0, 0.4, 0.25].into();
            let output = siren_node.tick(&input);
            outputs.push(output[0]);
        }

        let has_positive = outputs.iter().any(|&x| x > 0.01);
        let has_negative = outputs.iter().any(|&x| x < -0.01);

        assert!(
            has_positive && has_negative,
            "Should oscillate when input > 0"
        );
    }

    #[test]
    fn test_frequency_interpolation() {
        let mut siren_node = siren::<f32>();
        siren_node.reset();

        // Test that frequency is interpolated based on input amplitude
        // Test with a=0.5, frequency should be in the middle
        let mut zero_crossings = 0;
        let mut previous_output = 0.0;

        // Run for 2 seconds at 48kHz to account for initial low frequency
        for _ in 0..96000 {
            let input: Frame<f32, typenum::U5> = [0.3, 0.5, 775.0, 0.4, 0.25].into(); // Mid excitement level
            let output = siren_node.tick(&input);

            // Detect zero crossings (positive to negative)
            if previous_output > 0.0 && output[0] <= 0.0 {
                zero_crossings += 1;
            }
            previous_output = output[0];
        }

        // With a=0.5:
        // - Initial frequency: 0.5 Hz (takes ~1 second to first zero crossing)
        // - After first crossing: frequency = 0.5 + (15000 - 0.5) * 0.5 = 7500.25 Hz
        // - Pause duration after each crossing: 0.1 * (1 - 0.5) = 0.05 seconds
        // - With pauses, each cycle effectively takes: 1/7500 + 0.05 ≈ 0.05 seconds
        // - Effective frequency with pauses: ~20 Hz
        // - In 2 seconds, expect ~20-40 zero crossings (after initial slow period)
        assert!(
            (15..=45).contains(&zero_crossings),
            "With a=0.3 and pauses, expected 15-45 zero crossings in 2 seconds, got {}",
            zero_crossings
        );

        // Test with different input levels
        siren_node.reset();
        zero_crossings = 0;
        previous_output = 0.0;

        // Test with very low excitement (a=0.1) for 2 seconds
        for _ in 0..96000 {
            let input: Frame<f32, typenum::U5> = [0.1, 0.5, 775.0, 0.4, 0.25].into();
            let output = siren_node.tick(&input);

            if previous_output > 0.0 && output[0] <= 0.0 {
                zero_crossings += 1;
            }
            previous_output = output[0];
        }

        // With a=0.1:
        // - Frequency after first crossing: 0.5 + (15000 - 0.5) * 0.1 = 1500.05 Hz
        // - Pause duration: 0.1 * (1 - 0.1) = 0.09 seconds
        // - Each cycle = 1/1500 + 0.09 ≈ 0.09 seconds (effective ~11 Hz)
        assert!(
            (8..=25).contains(&zero_crossings),
            "With a=0.1 and longer pauses, expected 8-25 zero crossings in 2 seconds, got {}",
            zero_crossings
        );
    }
}
