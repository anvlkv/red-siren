use std::f32;

#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const NEW_YORK_ID: u64 = hash_str(concat!(module_path!(), "::NewYork"));

/// New York (parallel) compressor with soft knee compression.
///
/// Mixes a heavily-compressed signal with the dry signal to preserve dynamics while lifting quiet parts.
///
/// Dynamically computes the compression ratio based on threshold and input level.
///
/// - Input 0: audio signal
/// - Input 1: threshold (0.0 to 1.0)
/// - Input 2: wet mix (0.0 to 1.0)
/// - Output 0: compressed audio signal
#[derive(Default, Clone)]
pub struct NewYork<F: Real> {
    hash: u64,
    _phantom: core::marker::PhantomData<F>,
}

impl<F: Real> NewYork<F> {
    /// Create New York compressor with default settings
    pub fn new() -> Self {
        let mut compressor = NewYork::default();
        compressor.reset();
        compressor
    }

    /// Apply knee compression curve
    /// Small input levels are boosted, large ones are reduced
    #[inline]
    fn soft_compress(&self, x: F, t: F) -> F {
        let a = x.abs();

        // Aggression knob (higher => stronger flattening and more lift)
        let k_base = F::from_f32(40.0);
        // More aggressive when threshold is low
        let k = (F::one() - t) / t * k_base;

        // Target pivot below threshold to reduce overall amplitude
        // p_scale in (0,1). Try 0.75; lower for more reduction.
        let p_scale = F::from_f32(0.75);
        let p = t * p_scale;

        // Rational compander with pivot at p (not t):
        // y(a) = S * a / (1 + k a), with S chosen so y(t) = p
        let s = p * (F::one() + k * t) / t;
        let y = s * a / (F::one() + k * a);

        if x < F::zero() {
            -y
        } else {
            y
        }
    }
}

impl<F: Real> AudioNode for NewYork<F> {
    const ID: u64 = NEW_YORK_ID;
    type Inputs = typenum::U3;
    type Outputs = typenum::U1;

    fn reset(&mut self) {
        // No internal state to reset
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let dry_signal = F::from_f32(input[0]);

        // Get control parameters from inputs
        let threshold = F::from_f32(input[1].clamp(f32::EPSILON, 1.0));
        let wet_mix = F::from_f32(input[2].clamp(0.0, 1.0));

        // Apply compression to create wet signal
        let wet_signal = self.soft_compress(dry_signal, threshold);

        // Mix dry and wet signals
        // output = dry * (1 - mix) + wet * mix
        let dry_mix = F::one() - wet_mix;
        let output = dry_signal * dry_mix + wet_signal * wet_mix;

        [output.to_f32()].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Cache control parameters for this block
        let threshold = F::from_f32(input.at_f32(1, 0).clamp(f32::EPSILON, 1.0));
        let wet_mix = F::from_f32(input.at_f32(2, 0).clamp(0.0, 1.0));

        let dry_mix = F::one() - wet_mix;

        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let dry_signal = F::from_f32(input.at_f32(0, (i << SIMD_S) + j));

                // Apply compression
                let wet_signal = self.soft_compress(dry_signal, threshold);

                // Mix dry and wet
                let output = dry_signal * dry_mix + wet_signal * wet_mix;
                output.to_f32()
            });
            output.set(0, i, F32x::new(element));
        }

        self.process_remainder(size, input, output);
    }

    fn set_hash(&mut self, hash: u64) {
        self.hash = hash;
        self.reset();
    }
}

/// Create a New York compressor with default parameters
pub fn new_york<F>() -> An<NewYork<F>>
where
    F: Real,
{
    An(NewYork::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_new_york_compressor() {
        let node = (sine_hz::<f32>(440.0) * pass())
            >> split::<U2>()
            >> (pass() | constant(0.3) | constant(1.0) | pass())
            >> (new_york::<f32>() | pass());

        let config = SnapshotConfigBuilder::default()
            .num_samples(2000)
            .with_inputs(true)
            .show_grid(true)
            .background_color("#f0f0f0")
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(
            "new_york",
            node,
            InputSource::Generator(Box::new(|sample, _| match sample {
                ..50 => 0.0,
                50..250 => 0.001,
                250..500 => 0.01,
                500..750 => 0.1,
                750..1000 => 0.25,
                1000..1250 => 0.3,
                1250..1500 => 0.5,
                1500..1750 => 0.75,
                _ => 1.0,
            })),
            config
        );
    }
}
