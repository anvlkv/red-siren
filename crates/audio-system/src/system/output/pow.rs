use std::marker::PhantomData;

#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const POW_ID: u64 = hash_str(concat!(module_path!(), "::Pow"));

/// Simple audio node that converts signal to power of second signal.
#[derive(Clone)]
pub struct Pow<F: Real>(PhantomData<F>);

impl<F: Real> Pow<F> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<F: Real> Default for Pow<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Real> AudioNode for Pow<F> {
    const ID: u64 = POW_ID;
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
        let power: F = convert(input[1]);

        let value: F = signal.pow(power);

        [value.to_f32()].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let signal: F = convert(input.at_f32(0, (i << SIMD_S) + j));
                let power: F = convert(input.at_f32(1, (i << SIMD_S) + j));
                let value: F = signal.pow(power);

                value.to_f32()
            });
            output.set(0, i, F32x::new(element));
        }

        self.process_remainder(size, input, output);
    }
}

pub fn pow<F: Real>() -> An<Pow<F>> {
    An(Pow::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_pow() {
        let config = SnapshotConfigBuilder::default()
            .with_inputs(true)
            .show_grid(true)
            .num_samples(2100)
            .svg_height_per_channel(1000)
            .build()
            .unwrap();
        let node = (constant(2.0) | pass()) >> pow::<f32>();

        assert_audio_unit_snapshot!(
            "powers of 2",
            node,
            InputSource::Generator(Box::new(|s, _| match s {
                ..100 => -1.0,
                100..200 => -0.75,
                200..300 => -0.5,
                300..400 => -0.2,
                400..500 => -0.1,
                500..600 => 0.0,
                600..700 => 0.1,
                700..800 => 0.2,
                800..900 => 0.5,
                900..1000 => 0.75,
                1000..1100 => 1.0,
                1100..1200 => 1.1,
                1200..1300 => 1.2,
                1300..1400 => 1.5,
                1400..1500 => 1.75,
                1500..1600 => 2.0,
                1600..1700 => 2.1,
                1700..1800 => 2.5,
                1800..1900 => 2.75,
                _ => 3.0,
            })),
            config
        );
    }

    #[test]
    fn test_pow_frac() {
        let config = SnapshotConfigBuilder::default()
            .with_inputs(true)
            .show_grid(true)
            .num_samples(700)
            .build()
            .unwrap();
        let node = (constant(100.0 / 7.5) | pass()) >> pow::<f32>();

        assert_audio_unit_snapshot!(
            "powers of 1_33",
            node,
            InputSource::Generator(Box::new(|s, _| match s {
                ..100 => 0.0,
                100..200 => 0.1,
                200..300 => 0.2,
                300..400 => 0.5,
                400..500 => 0.75,
                500.. => 1.0,
            })),
            config
        );
    }
}
