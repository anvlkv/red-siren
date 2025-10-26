#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const ABS_ID: u64 = hash_str(concat!(module_path!(), "::Abs"));

/// Simple audio node that converts signal to absolute value.
/// One input, one output.
#[derive(Clone)]
pub struct Abs;

impl Abs {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Abs {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for Abs {
    const ID: u64 = ABS_ID;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        // No state to reset
    }

    fn set_sample_rate(&mut self, _sample_rate: f64) {
        // Sample rate independent
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        [input[0].abs()].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let input_sample = input.at_f32(0, (i << SIMD_S) + j);
                input_sample.abs()
            });
            output.set(0, i, F32x::new(element));
        }

        self.process_remainder(size, input, output);
    }
}

/// Create an absolute value node.
/// Converts input signal to its absolute value.
pub fn abs() -> An<Abs> {
    An(Abs::new())
}
