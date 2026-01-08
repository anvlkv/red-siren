use fundsp::prelude::*;

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
}

/// Create an absolute value node.
/// Converts input signal to its absolute value.
pub fn abs() -> An<Abs> {
    An(Abs::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_abs() {
        let node = sine_hz::<f32>(440.0) >> abs();

        assert_audio_unit_snapshot!(node);
    }
}
