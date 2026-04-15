use fundsp::{
    buffer::{BufferMut, BufferRef},
    prelude::*,
    signal::SignalFrame,
};

use crate::util::hash_str;

/// Stable node ID for `GeneratorNode` used by the fundsp graph.
const GENERATOR_NODE_ID: u64 = hash_str(concat!(module_path!(), "::GeneratorNode"));

/// A minimal single-channel sine wave oscillator used exclusively during
/// wavetable rendering inside [`super::QueuingAdsr`].
///
/// `GeneratorNode` is **not** inserted into a fundsp `Net`. It is driven
/// sample-by-sample via [`AudioUnit::tick`] while the wavetable is being
/// pre-rendered, and can optionally have its frequency overridden each tick
/// through `input[0]`.
#[derive(Clone)]
pub struct GeneratorNode {
    /// Current oscillator frequency in Hz.
    pub frequency: f32,
    /// Oscillator phase in the range `0.0..1.0`.
    phase: f64,
    /// Sample rate in Hz.
    sample_rate: f64,
}

impl GeneratorNode {
    /// Create a new `GeneratorNode` with a default frequency of 440 Hz.
    pub fn new(sample_rate: f64) -> Self {
        Self {
            frequency: 440.0,
            phase: 0.0,
            sample_rate,
        }
    }
}

impl AudioUnit for GeneratorNode {
    /// One input: when `input[0] > 0.0` it overrides the current frequency.
    fn inputs(&self) -> usize {
        1
    }

    /// One output: the current sine sample.
    fn outputs(&self) -> usize {
        1
    }

    /// Advance the oscillator by one sample.
    ///
    /// - If `input[0] > 0.0`, `self.frequency` is updated to that value.
    /// - The phase is advanced by `frequency / sample_rate`.
    /// - `output[0]` receives `sin(2π · phase)`.
    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        if input[0] > 0.0 {
            self.frequency = input[0];
        }
        self.phase = (self.phase + self.frequency as f64 / self.sample_rate).fract();
        output[0] = (self.phase * 2.0 * std::f64::consts::PI).sin() as f32;
    }

    /// Process a block of `size` scalar samples by calling [`Self::tick`] for each one.
    ///
    /// `size` is a scalar sample count (up to `MAX_BUFFER_SIZE` = 64). `BufferRef::at` /
    /// `BufferMut::at_mut` expect a **SIMD frame** index (0..size/8), so the outer loop
    /// runs over `size / 8` frames. Any remaining samples (when `size` is not a multiple
    /// of 8) are handled scalar-by-scalar via `at_f32` / `set_f32`.
    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        let simd_frames = size / 8;
        for s in 0..simd_frames {
            let freq_chunk: wide::f32x8 = input.at(0, s);
            let freqs = freq_chunk.to_array();
            let mut outs = [0.0_f32; 8];
            for k in 0..8 {
                let mut o = [0.0_f32];
                self.tick(&[freqs[k]], &mut o);
                outs[k] = o[0];
            }
            *output.at_mut(0, s) = wide::f32x8::from(outs);
        }
        // Handle remaining scalar samples when size is not a multiple of 8.
        for j in (simd_frames * 8)..size {
            let freq = input.at_f32(0, j);
            let mut o = [0.0_f32];
            self.tick(&[freq], &mut o);
            output.set_f32(0, j, o[0]);
        }
    }

    /// Update the stored sample rate.
    fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate;
    }

    /// Reset the oscillator phase to the start.
    fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// No heap resources to pre-allocate.
    fn allocate(&mut self) {}

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        SignalFrame::new(1)
    }

    fn get_id(&self) -> u64 {
        GENERATOR_NODE_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;

    #[test]
    fn inputs_and_outputs() {
        let node = GeneratorNode::new(SR);
        assert_eq!(node.inputs(), 1);
        assert_eq!(node.outputs(), 1);
    }

    #[test]
    fn tick_output_stays_within_unity() {
        let mut node = GeneratorNode::new(SR);
        let mut output = [0.0f32];

        for _ in 0..100 {
            node.tick(&[440.0], &mut output);
            assert!(
                output[0] >= -1.0 && output[0] <= 1.0,
                "sine output out of [-1, 1]: {}",
                output[0]
            );
        }
    }

    #[test]
    fn frequency_override_via_positive_input() {
        let mut node = GeneratorNode::new(SR);
        let mut output = [0.0f32];

        // Positive input should update frequency.
        node.tick(&[880.0], &mut output);
        assert_eq!(node.frequency, 880.0);
    }

    #[test]
    fn zero_input_keeps_existing_frequency() {
        let mut node = GeneratorNode::new(SR);
        node.frequency = 660.0;
        let mut output = [0.0f32];

        // Input of 0.0 must NOT override frequency.
        node.tick(&[0.0], &mut output);
        assert_eq!(node.frequency, 660.0);
    }

    #[test]
    fn reset_zeroes_phase() {
        let mut node = GeneratorNode::new(SR);
        let mut output = [0.0f32];

        // Advance a few frames so phase is non-zero.
        for _ in 0..10 {
            node.tick(&[440.0], &mut output);
        }
        node.reset();
        assert_eq!(node.phase, 0.0, "phase should be 0.0 after reset");
    }

    #[test]
    fn set_sample_rate_is_stored() {
        let mut node = GeneratorNode::new(SR);
        node.set_sample_rate(96_000.0);
        assert_eq!(node.sample_rate, 96_000.0);
    }

    #[test]
    fn footprint_is_nonzero() {
        let node = GeneratorNode::new(SR);
        assert!(node.footprint() > 0);
    }
}
