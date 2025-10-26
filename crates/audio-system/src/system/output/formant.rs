#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::{hash_str, S};

const FORMANT_ID: u64 = hash_str(concat!(module_path!(), "::Formant"));

#[derive(Clone)]
pub struct Formant<const D: u8> {
    base: S,
    control: Var,
    resonator: Resonator<f32, U3>,
}

impl<const D: u8> AudioNode for Formant<D> {
    const ID: u64 = FORMANT_ID;

    // Input 0: audio signal
    // Input 1: base Q value
    type Inputs = U2;

    type Outputs = U1;

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        // Map control to [0, 1]
        let v = self.control.value().clamp(0.0, 1.0) as S;

        // Get base Q from input
        let base_q = input[1] as S;

        // Reasonable Q mapping: broader at hight control value
        let q = (base_q - v * (base_q - S::EPSILON)) as f32;

        // Keep a consistent spacing between adjacent formants using semitone steps.
        // D indexes the formant band; apply a fixed step and a small detune from control.
        let step_semitones = 5.0; // distance between adjacent formants
        let detune_semitones = (v - 0.5) * 2.0; // +/- 1 semitone sweep by control
        let semitones = (D as S - 1.0) * step_semitones + detune_semitones;

        // Center frequency derived from base by semitone offset
        let center = (self.base * ((2.0 as S).powf(semitones / 12.0))) as f32;

        let result = self.resonator.tick(&[input[0], center, q].into());
        [result[0]].into()
    }

    fn reset(&mut self) {
        self.resonator.reset();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.resonator.set_sample_rate(sample_rate);
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Extract control value once per batch since it's shared
        let v = self.control.value().clamp(0.0, 1.0) as S;

        // Get base Q from input
        let base_q = input.at_f32(1, 0) as S;

        // Reasonable Q mapping: broader at high control value
        let q = (base_q - v * (base_q - S::EPSILON)) as f32;

        // Calculate frequency parameters once per batch
        let step_semitones = 5.0; // distance between adjacent formants
        let detune_semitones = (v - 0.5) * 2.0; // +/- 1 semitone sweep by control
        let semitones = (D as S - 1.0) * step_semitones + (detune_semitones * 120.0);

        // Center frequency derived from base by semitone offset
        let center = (self.base * ((2.0 as S).powf(semitones / 12.0))) as f32;

        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let input_sample = input.at_f32(0, (i << SIMD_S) + j);
                let resonator_input: Frame<f32, U3> = [input_sample, center, q].into();
                let result = self.resonator.tick(&resonator_input);
                result[0] as f32
            });
            output.set(0, i, F32x::new(element));
        }

        self.process_remainder(size, input, output);
    }
}

pub fn formant<const D: u8>(control: Var, base: S) -> An<Formant<D>> {
    let formant = Formant {
        control,
        base,
        resonator: Resonator::new(base as f32, 1.0),
    };

    An(formant)
}
