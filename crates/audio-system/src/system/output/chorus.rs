#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::{hash_str, S};

const CHORUS_ID: u64 = hash_str(concat!(module_path!(), "::Chorus"));

/// Simple circular delay buffer for chorus effects
#[derive(Clone, Copy)]
struct DelayBuffer<const N: usize, F: Real> {
    buffer: [F; N],
    write_pos: usize,
    limit: usize,
}

impl<const N: usize, F: Real> DelayBuffer<N, F> {
    fn new() -> Self {
        Self {
            buffer: [F::zero(); N],
            write_pos: 0,
            limit: N,
        }
    }

    fn write(&mut self, sample: F) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.limit;
    }

    fn read_at(&self, delay_samples: F) -> F {
        if delay_samples <= F::zero() {
            return F::zero();
        }

        let delay_samples = delay_samples.min(convert(self.limit as f32 - 1.0));
        let delay_int = delay_samples.floor().to_f32() as usize;
        let delay_frac = delay_samples - convert(delay_int as f32);

        // Calculate read positions - read_pos is the most recent position we can read
        // For a delay, we go back from the most recent written sample (write_pos - 1)
        let most_recent = if self.write_pos == 0 {
            self.limit - 1
        } else {
            self.write_pos - 1
        };

        let read_pos1 = (most_recent + self.limit - delay_int) % self.limit;
        let read_pos2 = (most_recent + self.limit - delay_int - 1) % self.limit;

        let sample1 = self.buffer[read_pos1];
        let sample2 = self.buffer[read_pos2];

        // Linear interpolation: when frac=0 we want sample1, when frac→1 we want to blend toward sample2
        sample1 * (F::one() - delay_frac) + sample2 * delay_frac
    }

    fn clear(&mut self) {
        self.buffer = [F::zero(); N];
        self.write_pos = 0;
    }

    fn limit(&mut self, max: usize) {
        if max > N {
            log::error!("Buffer size mismatch. Min required buffer size: [{max}] samples");
            self.limit = N;
        } else {
            self.limit = max;
        }
    }
}

pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 4;

/// Mono chorus with 5 voices * `N` stack.
///
/// Stacking reuses same delay buffer
///
/// - `D` max delay buffer size
/// - `N` number of stacked choruses
#[derive(Clone)]
pub struct Chorus<const D: usize, const N: usize, F: Real> {
    seed: [u64; N],
    separation: [F; N],
    variation: [F; N],
    mod_frequency: [F; N],

    // Delay buffers for 4 delayed voices (1 dry + 4 delayed = 5 voices total)
    delay_buffers: [DelayBuffer<D, F>; 4],

    // LFO phase accumulators for each voice
    lfo_phases: [[F; 4]; N],

    // Sample rate info
    sample_duration: F,
    sample_rate: f64,
}

impl<const D: usize, const N: usize, F: Real> Chorus<D, N, F> {
    pub fn new(
        seed: [u64; N],
        separation: [S; N],
        variation: [S; N],
        mod_frequency: [S; N],
    ) -> Self {
        let sample_rate = DEFAULT_SR;
        let sample_duration = convert(1.0 / sample_rate);

        Self {
            seed,
            separation: separation.map(convert),
            variation: variation.map(convert),
            mod_frequency: mod_frequency.map(convert),
            delay_buffers: [
                DelayBuffer::new(),
                DelayBuffer::new(),
                DelayBuffer::new(),
                DelayBuffer::new(),
            ],
            lfo_phases: [[F::zero(); 4]; N],
            sample_duration,
            sample_rate,
        }
    }

    /// Generate spline noise for LFO
    fn spline_noise(&self, seed: u64, t: F) -> F {
        spline_noise(seed, t)
    }

    /// Linear interpolation between -1 and 1 range to target range
    fn lerp11(&self, min: F, max: F, t: F) -> F {
        lerp11(min, max, t)
    }

    /// Hash function variants for different voices
    fn hash1(&self, seed: u64) -> u64 {
        hash1(seed)
    }

    fn hash2(&self, seed: u64) -> u64 {
        hash2(seed)
    }

    fn max_delay_samples(&self) -> usize {
        // Calculate maximum delay needed in samples
        (0..N)
            .map(|nth| {
                let max_delay_seconds = self.separation[nth] * convert(4.0) + self.variation[nth];
                (max_delay_seconds * convert(self.sample_rate))
                    .ceil()
                    .to_f32() as usize
                    + 1
            })
            .max()
            .unwrap_or_default()
    }

    #[inline]
    fn tick_internal(&mut self, input_sample: F) -> F {
        // Feed input to all delay buffers first
        for buffer in &mut self.delay_buffers {
            buffer.write(input_sample);
        }

        // Start with dry signal
        let mut output = input_sample;

        // iterate stack
        for nth in 0..N {
            // Generate 4 delayed voices
            for i in 0..4 {
                // Calculate LFO frequency with slight variations
                let lfo_freq = self.mod_frequency[nth] + convert(i as S * 0.02);

                // Generate delay time using spline noise
                let delay_time = match i {
                    0 => self.lerp11(
                        self.separation[nth],
                        self.separation[nth] + self.variation[nth],
                        self.spline_noise(self.seed[nth], self.lfo_phases[nth][i]),
                    ),
                    1 => self.lerp11(
                        self.separation[nth] * convert(2.0),
                        self.separation[nth] * convert(2.0) + self.variation[nth],
                        self.spline_noise(self.hash1(self.seed[nth]), self.lfo_phases[nth][i]),
                    ),
                    2 => self.lerp11(
                        self.separation[nth] * convert(3.0),
                        self.separation[nth] * convert(3.0) + self.variation[nth],
                        self.spline_noise(self.hash2(self.seed[nth]), self.lfo_phases[nth][i]),
                    ),
                    3 => self.lerp11(
                        self.separation[nth] * convert(4.0),
                        self.separation[nth] * convert(4.0) + self.variation[nth],
                        self.spline_noise(
                            self.hash1(self.seed[nth] ^ 0xfedcba),
                            self.lfo_phases[nth][i],
                        ),
                    ),
                    _ => unreachable!(),
                };

                // Convert delay time to samples and read from delay buffer
                let delay_samples: F = delay_time * convert(self.sample_rate);
                let delayed_sample = self.delay_buffers[i].read_at(delay_samples);

                // Add to output with scaling (0.2 per voice like in original)
                output += delayed_sample * convert(0.2);

                // Update LFO phase
                self.lfo_phases[nth][i] += lfo_freq * self.sample_duration;

                // Keep phase in reasonable range
                if self.lfo_phases[nth][i] > convert(1000.0) {
                    self.lfo_phases[nth][i] -= convert(1000.0);
                }
            }
            // write again after processing
            if nth < N - 1 {
                for buffer in &mut self.delay_buffers {
                    buffer.write(output);
                }
            }
        }

        output
    }
}

impl<const D: usize, const N: usize, F: Real> AudioNode for Chorus<D, N, F> {
    const ID: u64 = CHORUS_ID + (D + N * 16) as u64;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        for buffer in &mut self.delay_buffers {
            buffer.clear();
        }
        self.lfo_phases.fill([F::zero(); 4]);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.sample_duration = convert(1.0 / sample_rate);

        // Recreate delay buffers with new capacity for new sample rate
        let max_delay_samples = self.max_delay_samples();

        for buffer in &mut self.delay_buffers {
            *buffer = DelayBuffer::new();
            buffer.limit(max_delay_samples);
        }
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let output = self.tick_internal(convert(input[0]));
        [output.to_f32()].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Process SIMD blocks
        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let input_sample = input.at_f32(0, (i << SIMD_S) + j);
                self.tick_internal(convert(input_sample)).to_f32()
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
pub fn chorus<const N: usize>(
    seed: [u64; N],
    separation: [S; N],
    variation: [S; N],
    mod_frequency: [S; N],
) -> An<ChorusBank<N>> {
    An(Chorus::<DEFAULT_BUFFER_SIZE, N, S>::new(
        seed,
        separation,
        variation,
        mod_frequency,
    ))
}

pub type ChorusBank<const N: usize> = Chorus<DEFAULT_BUFFER_SIZE, N, S>;

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_chorus() {
        let node = sine_hz::<f32>(440.0)
            >> split::<U2>()
            >> (chorus([11, 12], [0.0003, 0.0005], [0.0015, 0.005], [1.2, 9.2]) | pass());

        assert_audio_unit_snapshot!(node);
    }
}
