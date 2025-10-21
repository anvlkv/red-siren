use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));
const SIREN_BASE_HZ: f32 = 0.5;
const MAX_FREQUENCY_HZ: f32 = 15000.0; // Maximum frequency before reversing direction
const FREQUENCY_GROWTH_FACTOR: f32 = 1.3; // Exponential growth factor

const BASE_PAUSE_DURATION: f32 = 0.1; // Base pause duration in seconds

/// Siren oscillator with excitement-controlled pauses and frequency doubling.
/// - Input 0: excitement level (a). Zero = silent, positive = oscillate with pauses.
/// - Output 0: siren wave with pauses and frequency doubling.
#[derive(Default, Clone)]
pub struct Siren<F: Real> {
    freq: F,
    phase: F,
    sample_duration: F,
    pause_timer: F,          // Tracks remaining pause time
    previous_sine: F,        // For zero-crossing detection
    freq_direction_up: bool, // True = increasing freq, False = decreasing freq
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
    fn calculate_pause_duration(&self, a: F) -> F {
        if a > F::zero() {
            // Higher amplitude = shorter pause
            let pause_factor = (F::one() - a).max(F::zero());
            F::from_f32(BASE_PAUSE_DURATION) * pause_factor
        } else {
            F::zero()
        }
    }
}

impl<F: Real> AudioNode for Siren<F> {
    const ID: u64 = SIREN_ID;
    type Inputs = typenum::U1;
    type Outputs = typenum::U1;

    fn reset(&mut self) {
        self.phase = F::zero();
        self.pause_timer = F::zero();
        self.previous_sine = F::zero();
        self.freq = F::from_f32(SIREN_BASE_HZ);
        self.freq_direction_up = true;
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_duration = convert(1.0 / sample_rate);
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let a = F::from_f32(input[0]);

        // Silent when input is zero
        // Resets freq and direction
        if a <= F::zero() {
            self.pause_timer = F::zero();
            self.previous_sine = F::zero();
            self.freq = F::from_f32(SIREN_BASE_HZ);
            self.freq_direction_up = true;
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
            // Update frequency exponentially with direction
            if self.freq_direction_up {
                self.freq *= F::from_f32(FREQUENCY_GROWTH_FACTOR);
                // Check if we've hit the maximum frequency
                if self.freq.to_f32() >= MAX_FREQUENCY_HZ {
                    self.freq = F::from_f32(MAX_FREQUENCY_HZ);
                    self.freq_direction_up = false; // Start going down
                }
            } else {
                self.freq /= F::from_f32(FREQUENCY_GROWTH_FACTOR);
                // Check if we've hit the minimum frequency
                if self.freq.to_f32() <= SIREN_BASE_HZ {
                    self.freq = F::from_f32(SIREN_BASE_HZ);
                    self.freq_direction_up = true; // Start going up again
                }
            }

            // Calculate and set pause duration
            self.pause_timer = self.calculate_pause_duration(a);

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
        let mut freq_direction_up = self.freq_direction_up;

        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let a = F::from_f32(input.at_f32(0, (i << SIMD_S) + j));

                // Silent when input is zero
                if a <= F::zero() {
                    pause_timer = F::zero();
                    previous_sine = F::zero();
                    freq = F::from_f32(SIREN_BASE_HZ);
                    freq_direction_up = true;
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
                    // Update frequency exponentially with direction
                    if freq_direction_up {
                        freq *= F::from_f32(FREQUENCY_GROWTH_FACTOR);
                        // Check if we've hit the maximum frequency
                        if freq.to_f32() >= MAX_FREQUENCY_HZ {
                            freq = F::from_f32(MAX_FREQUENCY_HZ);
                            freq_direction_up = false; // Start going down
                        }
                    } else {
                        freq /= F::from_f32(FREQUENCY_GROWTH_FACTOR);
                        // Check if we've hit the minimum frequency
                        if freq.to_f32() <= SIREN_BASE_HZ {
                            freq = F::from_f32(SIREN_BASE_HZ);
                            freq_direction_up = true; // Start going up again
                        }
                    }

                    // Calculate pause duration
                    pause_timer = if a > F::zero() {
                        let pause_factor = (F::one() - a).max(F::zero());
                        F::from_f32(BASE_PAUSE_DURATION) * pause_factor
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
        self.freq_direction_up = freq_direction_up;
        self.process_remainder(size, input, output);
    }

    fn set_hash(&mut self, hash: u64) {
        self.hash = hash;
        self.reset();
    }
}

pub fn siren() -> An<Siren<f32>> {
    let siren = Siren::new();
    An(siren)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_siren_behavior() {
        // Test with zero input
        let mut siren_node = siren();
        siren_node.reset();

        let mut outputs = Vec::new();

        // Process 2 seconds worth of samples with zero input
        for _ in 0..96000 {
            let input: Frame<f32, typenum::U1> = [0.0].into();
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
            let input: Frame<f32, typenum::U1> = [0.3].into();
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
    fn test_frequency_growth_and_oscillation() {
        let mut siren_node = siren();
        siren_node.reset();

        // Test that frequency grows exponentially and then oscillates up/down
        let mut zero_crossings = 0;
        let mut previous_output = 0.0;
        let mut non_zero_outputs = 0;

        for _i in 0..480000 {
            // 10 seconds at 48kHz to see full oscillation pattern (up to 15kHz then down)
            let input: Frame<f32, typenum::U1> = [1.0].into(); // Maximum excitement to eliminate pauses
            let output = siren_node.tick(&input);

            if output[0] != 0.0 {
                non_zero_outputs += 1;
            }

            // Detect zero crossings (positive to negative)
            if previous_output > 0.0 && output[0] <= 0.0 {
                zero_crossings += 1;
            }
            previous_output = output[0];
        }

        // With exponential growth factor 1.3 and no pauses:
        // Frequency grows: 0.5 -> 0.65 -> 0.845 -> 1.1 -> 1.43 -> 1.86 -> 2.42 -> 3.15 -> 4.09 -> 5.32 -> 6.92 -> 9.0 -> 11.7 -> 15.2
        // Then reverses direction and goes back down, creating oscillation pattern
        // Over 10 seconds we should see many crossings as frequency oscillates up and down
        assert!(
            zero_crossings >= 10,
            "Should have many zero crossings for full frequency oscillation, got {}",
            zero_crossings
        );
        assert!(
            non_zero_outputs > 400000,
            "Should have substantial non-zero output over 10 seconds, got {}",
            non_zero_outputs
        );
    }
}
