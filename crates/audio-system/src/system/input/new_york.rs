#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const NEW_YORK_ID: u64 = hash_str(concat!(module_path!(), "::NewYork"));

/// New York (parallel) compressor with soft knee compression.
/// Mixes a heavily-compressed signal with the dry signal to preserve dynamics while lifting quiet parts.
///
/// - Input 0: audio signal
/// - Input 1: threshold (0.0 to 1.0)
/// - Input 2: ratio (1.0 to 10.0)
/// - Input 3: wet mix (0.0 to 1.0)
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

    /// Apply soft knee compression curve
    /// Small input levels are slightly boosted, large ones are reduced
    #[inline]
    fn soft_compress(&self, x: F, threshold: F, ratio: F) -> F {
        let abs_x = x.abs();

        let gain = if abs_x > threshold {
            // Above threshold: attenuate
            // Output = threshold + (input - threshold) / ratio
            threshold + (abs_x - threshold) / ratio
        } else {
            // Below threshold: slightly boost
            // Use a smooth transition with (2.0 - ratio/ratio_max) factor
            // This gives a subtle boost that decreases as ratio increases
            let boost_factor = F::from_f32(2.0) - ratio / F::from_f32(10.0);
            abs_x * boost_factor
        };

        // Restore original sign
        if x < F::zero() {
            -gain
        } else {
            gain
        }
    }
}

impl<F: Real> AudioNode for NewYork<F> {
    const ID: u64 = NEW_YORK_ID;
    type Inputs = typenum::U4;
    type Outputs = typenum::U1;

    fn reset(&mut self) {
        // No internal state to reset
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let dry_signal = F::from_f32(input[0]);

        // Get control parameters from inputs
        let threshold = F::from_f32(input[1].clamp(0.0, 1.0));
        let ratio = F::from_f32(input[2].clamp(1.0, 10.0));
        let wet_mix = F::from_f32(input[3].clamp(0.0, 1.0));

        // Apply compression to create wet signal
        let wet_signal = self.soft_compress(dry_signal, threshold, ratio);

        // Mix dry and wet signals
        // output = dry * (1 - mix) + wet * mix
        let dry_mix = F::one() - wet_mix;
        let output = dry_signal * dry_mix + wet_signal * wet_mix;

        [output.to_f32()].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Cache control parameters for this block
        let threshold = F::from_f32(input.at_f32(1, 0).clamp(0.0, 1.0));
        let ratio = F::from_f32(input.at_f32(2, 0).clamp(1.0, 10.0));
        let wet_mix = F::from_f32(input.at_f32(3, 0).clamp(0.0, 1.0));

        let dry_mix = F::one() - wet_mix;

        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let dry_signal = F::from_f32(input.at_f32(0, (i << SIMD_S) + j));

                // Apply compression
                let wet_signal = self.soft_compress(dry_signal, threshold, ratio);

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

pub type StaticNewYork<F> =
    Pipe<Stack<Stack<Stack<Pass, Constant<U1>>, Constant<U1>>, Constant<U1>>, NewYork<F>>;

/// Create a New York compressor with custom parameters
pub fn new_york_with<F>(threshold: f32, ratio: f32, wet_mix: f32) -> An<StaticNewYork<F>>
where
    F: Real,
{
    (pass() | constant(threshold) | constant(ratio) | constant(wet_mix)) >> An(NewYork::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_york_passthrough() {
        let mut compressor = new_york::<f32>();
        compressor.reset();

        // With wet_mix = 0, should pass through dry signal
        let input: Frame<f32, typenum::U4> = [0.5, 0.3, 4.0, 0.0].into();
        let output = compressor.tick(&input);

        assert!(
            (output[0] - 0.5).abs() < 0.001,
            "With wet_mix=0, should pass through dry signal"
        );
    }

    #[test]
    fn test_new_york_compression() {
        let mut compressor = new_york::<f32>();
        compressor.reset();

        // Test with full wet mix
        let input: Frame<f32, typenum::U4> = [0.8, 0.3, 4.0, 1.0].into();
        let output = compressor.tick(&input);

        // Input above threshold (0.8 > 0.3) should be compressed
        // Expected: 0.3 + (0.8 - 0.3) / 4.0 = 0.3 + 0.125 = 0.425
        assert!(
            output[0] < 0.8,
            "Large signals should be compressed, got {}",
            output[0]
        );

        // Test with small signal (boost)
        let input: Frame<f32, typenum::U4> = [0.1, 0.3, 4.0, 1.0].into();
        let output = compressor.tick(&input);

        // Input below threshold should be slightly boosted
        // Boost factor = 2.0 - 4.0/10.0 = 1.6
        // Expected: 0.1 * 1.6 = 0.16
        assert!(
            output[0] > 0.1,
            "Small signals should be boosted, got {}",
            output[0]
        );
    }

    #[test]
    fn test_new_york_mixing() {
        let mut compressor = new_york::<f32>();
        compressor.reset();

        // Test 50/50 mix
        let input: Frame<f32, typenum::U4> = [1.0, 0.3, 4.0, 0.5].into();
        let output = compressor.tick(&input);

        // Dry = 1.0
        // Wet (compressed) = 0.3 + (1.0 - 0.3) / 4.0 = 0.475
        // Mixed = 1.0 * 0.5 + 0.475 * 0.5 = 0.7375
        assert!(
            (output[0] - 0.7375).abs() < 0.01,
            "50/50 mix should blend dry and wet, got {}",
            output[0]
        );
    }

    #[test]
    fn test_negative_signals() {
        let mut compressor = new_york::<f32>();
        compressor.reset();

        // Test with negative input
        let input: Frame<f32, typenum::U4> = [-0.8, 0.3, 4.0, 1.0].into();
        let output = compressor.tick(&input);

        // Should preserve sign
        assert!(
            output[0] < 0.0,
            "Should preserve negative sign, got {}",
            output[0]
        );

        // Magnitude should be compressed same as positive
        assert!(
            output[0].abs() < 0.8,
            "Negative signals should be compressed by magnitude, got {}",
            output[0]
        );
    }
}
