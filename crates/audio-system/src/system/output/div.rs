use std::marker::PhantomData;

#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const DIV_ID: u64 = hash_str(concat!(module_path!(), "::Div"));

/// Simple audio node that divides input signal by second signal.
#[derive(Clone)]
pub struct Div<F: Real>(PhantomData<F>);

impl<F: Real> Div<F> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<F: Real> Default for Div<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Real> AudioNode for Div<F> {
    const ID: u64 = DIV_ID;
    type Inputs = U2;
    type Outputs = U1;

    fn reset(&mut self) {
        // No state to reset
    }

    fn set_sample_rate(&mut self, _sample_rate: f64) {
        // Sample rate independent
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let signal: F = convert(input[0]);
        let divisor: F = convert(input[1]);

        let value: F = signal / divisor;

        [value.to_f32()].into()
    }
}

pub fn div<F: Real>() -> An<Div<F>> {
    An(Div::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_div() {
        let config = SnapshotConfigBuilder::default()
            .show_grid(true)
            .num_samples(4)
            .svg_width(400)
            .build()
            .unwrap();
        let node = (constant(100.0) | constant(5.0)) >> div::<f32>();

        assert_audio_unit_snapshot!("100 by 5", node, InputSource::None, config);
    }
}
