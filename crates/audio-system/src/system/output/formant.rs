#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::{hash_str, S};

const FORMANT_ID: u64 = hash_str(concat!(module_path!(), "::Formant"));

#[derive(Clone)]
pub struct Formant<const D: u8> {
    base: S,
    resonator: Resonator<f32, U3>,
    steps: u32,
}

impl<const D: u8> Formant<D> {
    #[inline]
    fn tick_internal(&mut self, audio_sample: f32, control: f32, base_q: f32) -> f32 {
        // Map control to [0, 1]
        let v = control as S;

        // Get base Q
        let base_q = base_q as S;

        // Reasonable Q mapping: broader at high control value
        let q = (base_q - v * (base_q - S::EPSILON)) as f32;

        // Keep a consistent spacing between adjacent formants using semitone steps.
        // D indexes the formant band; apply a fixed step and a small detune from control.
        let step_semitones = self.steps as S; // distance between adjacent formants
        let detune_semitones = (v - 0.5) * 2.0; // +/- 1 semitone sweep by control
        let semitones = (D as S - 1.0) * step_semitones + detune_semitones;

        // Center frequency derived from base by semitone offset
        let center =
            (self.base * ((2.0 as S).powf(semitones / ((self.steps * 3) as S / 2.0)))) as f32;

        let result = self.resonator.tick(&[audio_sample, center, q].into());
        result[0]
    }
}

#[allow(clippy::unnecessary_cast)]
impl<const D: u8> AudioNode for Formant<D> {
    const ID: u64 = FORMANT_ID;

    // Input 0: audio signal
    // Input 1: control
    // Input 2: base Q value
    type Inputs = U3;

    type Outputs = U1;

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let output = self.tick_internal(input[0], input[1], input[2]);
        [output].into()
    }

    fn reset(&mut self) {
        self.resonator.reset();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.resonator.set_sample_rate(sample_rate);
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let idx = (i << SIMD_S) + j;
                let input_sample = input.at_f32(0, idx);
                let control = input.at_f32(1, idx);
                let base_q = input.at_f32(2, idx);
                self.tick_internal(input_sample, control, base_q)
            });
            output.set(0, i, F32x::new(element));
        }

        self.process_remainder(size, input, output);
    }
}

#[allow(clippy::unnecessary_cast)]
pub fn formant<const D: u8>(base: S, steps: u32) -> An<Formant<D>> {
    let formant = Formant {
        base,
        steps,
        resonator: Resonator::new(base as f32, 1.0),
    };

    An(formant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_formant() {
        let config = SnapshotConfig::default();
        let node = (saw_hz(440.0) | pass() | pass()) >> formant::<1>(440.0, 7);

        assert_audio_unit_snapshot!(
            "formant_1",
            node,
            InputSource::Generator(Box::new(|sample, ch| match ch {
                0 => match sample {
                    ..250 => 0.0,
                    250..500 => 0.5,
                    500..750 => 0.75,
                    _ => 1.0,
                },
                _ => 1.0,
            })),
            config
        );
    }

    #[test]
    fn test_formants_chain() {
        let config = SnapshotConfig::default();
        let node = (saw_hz(440.0) | pass() | pass() | pass() | pass() | pass() | pass())
            >> (formant::<1>(440.0, 7) | pass() | pass() | pass() | pass())
            >> (formant::<2>(440.0, 7) | pass() | pass())
            >> formant::<3>(440.0, 7);

        assert_audio_unit_snapshot!(
            "formants_chain",
            node,
            InputSource::Generator(Box::new(|sample, ch| match ch {
                0 | 2 | 4 => match sample {
                    ..250 => 0.0,
                    250..500 => 0.5,
                    500..750 => 0.75,
                    _ => 1.0,
                },
                _ => 1.0,
            })),
            config
        );
    }
}
