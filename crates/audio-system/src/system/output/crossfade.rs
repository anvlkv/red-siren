use std::f32::consts::PI;

#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const CROSSFADE_ID: u64 = hash_str(concat!(module_path!(), "::EqualPowerCrossfade"));

/// Equal power crossfade node.
///
/// - Input 0: Audio signal from source 1
/// - Input 1: Audio signal from source 2
/// - Input 2: Crossfade amount (0.0 = full source 1, 1.0 = full source 2)
/// - Output 0: Crossfaded audio
///
/// Uses equal power (constant power) crossfading formula:
/// - Gain 1 = cos(fade * π/2)
/// - Gain 2 = sin(fade * π/2)
///
/// This ensures constant perceived loudness when crossfading between uncorrelated signals.
#[derive(Clone)]
pub struct EqualPowerCrossfade {
    _marker: std::marker::PhantomData<f32>,
}

impl EqualPowerCrossfade {
    /// Create a new equal power crossfade node.
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl Default for EqualPowerCrossfade {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for EqualPowerCrossfade {
    const ID: u64 = CROSSFADE_ID;

    // Type-level specification: 3 inputs, 1 output
    type Inputs = U3;
    type Outputs = U1;

    // Reset the node state
    fn reset(&mut self) {
        // This node is stateless, so nothing to reset
    }

    // Set sample rate (we don't need it for this node)
    fn set_sample_rate(&mut self, _sample_rate: f64) {
        // This node doesn't depend on sample rate
    }

    // Process a single sample
    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        // Extract inputs
        let signal1 = input[0];
        let signal2 = input[1];
        let fade = input[2];

        // Calculate equal power gains using sin/cos
        let angle = fade * PI * 0.5;
        let gain1 = angle.cos();
        let gain2 = angle.sin();

        // Apply gains and mix
        let output = signal1 * gain1 + signal2 * gain2;

        // Return output frame
        [output].into()
    }
}

// Convenience function to create the node
/// Create an equal power crossfade node.
///
/// - Input 0: Audio signal from source 1
/// - Input 1: Audio signal from source 2
/// - Input 2: Crossfade amount (0.0 to 1.0)
/// - Output 0: Crossfaded audio
pub fn equal_power_crossfade() -> An<EqualPowerCrossfade> {
    An(EqualPowerCrossfade::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_crossfade() {
        let config = SnapshotConfigBuilder::default()
            .num_samples(2000)
            .with_inputs(true)
            .build()
            .unwrap();

        let node = (sine_hz::<f32>(440.0) | saw_hz(440.0) | pass()) >> equal_power_crossfade();

        assert_audio_unit_snapshot!(
            "crossfade_0_1",
            node,
            InputSource::Generator(Box::new(|sample, _| match sample {
                0..250 => 0.0,
                250..500 => 0.1,
                500..750 => 0.25,
                750..1000 => 0.5,
                1000..1250 => 0.75,
                1250..1500 => 0.9,
                _ => 1.0,
            })),
            config
        );
    }

    #[test]
    fn test_crossfade_process() {
        let config = SnapshotConfigBuilder::default()
            .processing_mode(Processing::Batch(24))
            .num_samples(2000)
            .with_inputs(true)
            .build()
            .unwrap();

        let node = (sine_hz::<f32>(440.0) | saw_hz(440.0) | pass()) >> equal_power_crossfade();

        assert_audio_unit_snapshot!(
            "crossfade_process_0_1",
            node,
            InputSource::Generator(Box::new(|sample, _| match sample {
                0..250 => 0.0,
                250..500 => 0.1,
                500..750 => 0.25,
                750..1000 => 0.5,
                1000..1250 => 0.75,
                1250..1500 => 0.9,
                _ => 1.0,
            })),
            config
        );
    }
}
