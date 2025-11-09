#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const CHORUS_ID: u64 = hash_str(concat!(module_path!(), "::Chorus"));

/// Simple circular delay buffer for chorus effects
#[derive(Clone)]
struct DelayBuffer {
    buffer: Vec<f32>,
    write_pos: usize,
}

impl DelayBuffer {
    fn new(max_delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay_samples],
            write_pos: 0,
        }
    }

    fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
    }

    fn read_at(&self, delay_samples: f32) -> f32 {
        if delay_samples <= 0.0 {
            return 0.0;
        }

        let delay_samples = delay_samples.min(self.buffer.len() as f32 - 1.0);
        let delay_int = delay_samples.floor() as usize;
        let delay_frac = delay_samples - delay_int as f32;

        // Calculate read positions - read_pos is the most recent position we can read
        // For a delay, we go back from the most recent written sample (write_pos - 1)
        let most_recent = if self.write_pos == 0 {
            self.buffer.len() - 1
        } else {
            self.write_pos - 1
        };

        let read_pos1 = (most_recent + self.buffer.len() - delay_int) % self.buffer.len();
        let read_pos2 = (most_recent + self.buffer.len() - delay_int - 1) % self.buffer.len();

        let sample1 = self.buffer[read_pos1];
        let sample2 = self.buffer[read_pos2];

        // Linear interpolation: when frac=0 we want sample1, when frac→1 we want to blend toward sample2
        sample1 * (1.0 - delay_frac) + sample2 * delay_frac
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}

/// Mono chorus with 5 voices.
#[derive(Clone)]
pub struct Chorus {
    seed: u64,
    separation: f32,
    variation: f32,
    mod_frequency: f32,

    // Delay buffers for 4 delayed voices (1 dry + 4 delayed = 5 voices total)
    delay_buffers: [DelayBuffer; 4],

    // LFO phase accumulators for each voice
    lfo_phases: [f32; 4],

    // Sample rate info
    sample_duration: f32,
    sample_rate: f64,
}

impl Chorus {
    pub fn new(seed: u64, separation: f32, variation: f32, mod_frequency: f32) -> Self {
        let sample_rate = DEFAULT_SR;
        let sample_duration = 1.0 / sample_rate as f32;

        // Calculate maximum delay needed in samples
        let max_delay_seconds = separation * 4.0 + variation;
        let max_delay_samples = (max_delay_seconds * sample_rate as f32).ceil() as usize + 1;

        Self {
            seed,
            separation,
            variation,
            mod_frequency,
            delay_buffers: [
                DelayBuffer::new(max_delay_samples),
                DelayBuffer::new(max_delay_samples),
                DelayBuffer::new(max_delay_samples),
                DelayBuffer::new(max_delay_samples),
            ],
            lfo_phases: [0.0; 4],
            sample_duration,
            sample_rate,
        }
    }

    /// Generate spline noise for LFO
    fn spline_noise(&self, seed: u64, t: f32) -> f32 {
        spline_noise(seed, t)
    }

    /// Linear interpolation between -1 and 1 range to target range
    fn lerp11(&self, min: f32, max: f32, t: f32) -> f32 {
        lerp11(min, max, t)
    }

    /// Hash function variants for different voices
    fn hash1(&self, seed: u64) -> u64 {
        hash1(seed)
    }

    fn hash2(&self, seed: u64) -> u64 {
        hash2(seed)
    }
}

impl Chorus {
    #[inline]
    fn tick_internal(&mut self, input_sample: f32) -> f32 {
        // Feed input to all delay buffers first
        for buffer in &mut self.delay_buffers {
            buffer.write(input_sample);
        }

        // Start with dry signal
        let mut output = input_sample;

        // Generate 4 delayed voices
        for i in 0..4 {
            // Calculate LFO frequency with slight variations
            let lfo_freq = self.mod_frequency + (i as f32 * 0.02);

            // Generate delay time using spline noise
            let delay_time = match i {
                0 => self.lerp11(
                    self.separation,
                    self.separation + self.variation,
                    self.spline_noise(self.seed, self.lfo_phases[i]),
                ),
                1 => self.lerp11(
                    self.separation * 2.0,
                    self.separation * 2.0 + self.variation,
                    self.spline_noise(self.hash1(self.seed), self.lfo_phases[i]),
                ),
                2 => self.lerp11(
                    self.separation * 3.0,
                    self.separation * 3.0 + self.variation,
                    self.spline_noise(self.hash2(self.seed), self.lfo_phases[i]),
                ),
                3 => self.lerp11(
                    self.separation * 4.0,
                    self.separation * 4.0 + self.variation,
                    self.spline_noise(self.hash1(self.seed ^ 0xfedcba), self.lfo_phases[i]),
                ),
                _ => unreachable!(),
            };

            // Convert delay time to samples and read from delay buffer
            let delay_samples = delay_time * self.sample_rate as f32;
            let delayed_sample = self.delay_buffers[i].read_at(delay_samples);

            // Add to output with scaling (0.2 per voice like in original)
            output += delayed_sample * 0.2;

            // Update LFO phase
            self.lfo_phases[i] += lfo_freq * self.sample_duration;

            // Keep phase in reasonable range
            if self.lfo_phases[i] > 1000.0 {
                self.lfo_phases[i] -= 1000.0;
            }
        }

        output
    }
}

impl AudioNode for Chorus {
    const ID: u64 = CHORUS_ID;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        for buffer in &mut self.delay_buffers {
            buffer.clear();
        }
        self.lfo_phases.fill(0.0);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.sample_duration = 1.0 / sample_rate as f32;

        // Recreate delay buffers with new capacity for new sample rate
        let max_delay_seconds = self.separation * 4.0 + self.variation;
        let max_delay_samples = (max_delay_seconds * sample_rate as f32).ceil() as usize + 1;

        for buffer in &mut self.delay_buffers {
            *buffer = DelayBuffer::new(max_delay_samples);
        }
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let output = self.tick_internal(input[0]);
        [output].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Process SIMD blocks
        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let input_sample = input.at_f32(0, (i << SIMD_S) + j);
                self.tick_internal(input_sample)
            });
            output.set(0, i, F32x::new(element));
        }

        // Process remainder
        self.process_remainder(size, input, output);
    }
}

/// Create a mono chorus with 5 voices.
/// `seed`: LFO seed for randomization.
/// `separation`: base voice separation in seconds (e.g., 0.015).
/// `variation`: delay variation in seconds (e.g., 0.005).
/// `mod_frequency`: delay modulation frequency (e.g., 0.2).
pub fn chorus(seed: u64, separation: f32, variation: f32, mod_frequency: f32) -> An<Chorus> {
    An(Chorus::new(seed, separation, variation, mod_frequency))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_chorus() {
        let node = sine_hz::<f32>(440.0) >> split::<U2>() >> (chorus(1, 0.15, 0.5, 1.2) | pass());

        assert_audio_unit_snapshot!(node);
    }
}
