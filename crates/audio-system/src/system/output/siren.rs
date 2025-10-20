use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));
const SIREN_BASE_HZ: f32 = 0.5;
const PAUSE_THRESHOLD: f32 = 0.5; // Above this, pause becomes negative
const BASE_PAUSE_DURATION: f32 = 0.1; // Base pause duration in seconds

#[derive(Clone)]
pub struct Siren {
    freq: f32,
    phase: f32,
    sample_duration: f32,
    pause_timer: f32,   // Tracks remaining pause time
    previous_sine: f32, // For zero-crossing detection
}

impl Siren {
    /// Detects if we've crossed zero between previous and current sine values
    fn has_zero_crossed(&self, current_sine: f32) -> bool {
        // Zero crossing occurs when signs differ and we're going from positive to negative
        self.previous_sine > 0.0 && current_sine <= 0.0
    }

    /// Calculates pause duration based on input amplitude
    /// Returns negative duration when a > PAUSE_THRESHOLD
    fn calculate_pause_duration(&self, a: f32) -> f32 {
        if a > 0.0 {
            // Higher amplitude = shorter pause
            // When a > PAUSE_THRESHOLD, this becomes negative
            BASE_PAUSE_DURATION * (1.0 - a / PAUSE_THRESHOLD)
        } else {
            0.0
        }
    }
}

impl AudioNode for Siren {
    const ID: u64 = SIREN_ID;

    type Inputs = U1;
    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let a = input[0];

        // Silent when input is zero
        if a <= 0.0 {
            self.pause_timer = 0.0;
            self.previous_sine = 0.0;
            return [0.0].into();
        }

        // Handle pause state
        if self.pause_timer > 0.0 {
            // Still pausing
            self.pause_timer -= self.sample_duration;
            return [0.0].into();
        }

        // Negative pause means we skip the pause entirely and continue oscillating
        if self.pause_timer < 0.0 {
            self.pause_timer = 0.0;
        }

        // Generate sine wave
        let current_sine = self.phase.sin();

        // Check for zero crossing (positive to negative)
        if self.has_zero_crossed(current_sine) {
            // Calculate and set pause duration
            self.pause_timer = self.calculate_pause_duration(a);

            // If pause is negative (a > PAUSE_THRESHOLD), we don't actually pause
            if self.pause_timer > 0.0 {
                self.previous_sine = current_sine;
                return [0.0].into();
            }
        }

        // Advance phase
        let two_pi = std::f32::consts::TAU;
        let omega = two_pi * self.freq;
        self.phase += omega * self.sample_duration;

        // Wrap phase to [0, 2π)
        if self.phase >= two_pi {
            self.phase -= two_pi;
        }

        // Store current sine for next zero-crossing check
        self.previous_sine = current_sine;

        [current_sine].into()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.pause_timer = 0.0;
        self.previous_sine = 0.0;
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_duration = 1.0 / sample_rate as f32;
    }
}

pub fn siren() -> An<Siren> {
    let mut siren = Siren {
        freq: SIREN_BASE_HZ,
        phase: 0.0,
        sample_duration: 1.0 / DEFAULT_SR as f32,
        pause_timer: 0.0,
        previous_sine: 0.0,
    };
    siren.reset();
    siren.set_sample_rate(DEFAULT_SR);
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
            let input: Frame<f32, U1> = [0.0].into();
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
            let input: Frame<f32, U1> = [0.3].into();
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
}
