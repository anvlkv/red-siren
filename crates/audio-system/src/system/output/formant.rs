#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::{hash_str, S};

const FORMANT_ID: u64 = hash_str(concat!(module_path!(), "::Formant"));

#[derive(Clone)]
pub struct Formant {
    base: S,
    resonator: Resonator<S, U3>,
}

impl Formant {
    fn resonator_params(audio_sample: S, control: S, base_q: S, base_freq: S) -> [S; 3] {
        let q = base_q + control * base_q * (1.0 / base_q);
        let center = base_freq;

        [audio_sample, center, q]
    }
}

#[allow(clippy::unnecessary_cast)]
impl AudioNode for Formant {
    const ID: u64 = FORMANT_ID as u64;

    // Input 0: audio signal
    // Input 1: control
    // Input 2: base Q value
    type Inputs = U3;

    type Outputs = U1;

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let params = Self::resonator_params(input[0] as S, input[1] as S, input[2] as S, self.base);
        self.resonator
            .tick(&[convert(params[0]), convert(params[1]), convert(params[2])].into())
    }

    fn reset(&mut self) {
        self.resonator.reset();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.resonator.set_sample_rate(sample_rate);
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        for i in 0..size {
            let input_sample = input.at_f32(0, i);
            let control = input.at_f32(1, i);
            let base_q = input.at_f32(2, i);
            let element =
                Self::resonator_params(input_sample as S, control as S, base_q as S, self.base);

            let tick = self.resonator.tick(
                &[
                    convert(element[0]),
                    convert(element[1]),
                    convert(element[2]),
                ]
                .into(),
            );

            output.set_f32(0, i, tick[0]);
        }
    }
}

#[allow(clippy::unnecessary_cast)]
pub fn formant(base: S) -> An<Formant> {
    let formant = Formant {
        base,
        resonator: Resonator::new(base, 1.0 as S),
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
        let node = (saw_hz(440.0) | pass() | pass()) >> formant(440.0);

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
    fn test_formant_process() {
        let config = SnapshotConfigBuilder::default()
            .processing_mode(Processing::Batch(64))
            .build()
            .unwrap();
        let node = (saw_hz(440.0) | pass() | pass()) >> formant(440.0);

        assert_audio_unit_snapshot!(
            "formant_1_process",
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
            >> (formant(440.0) | pass() | pass() | pass() | pass())
            >> (formant(440.0) | pass() | pass())
            >> formant(440.0);

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

    #[test]
    fn test_formants_stack_join() {
        let config = SnapshotConfig::default();
        let node = (((saw_hz(440.0) | pass() | pass()) >> formant(440.0))
            | ((saw_hz(440.0) | pass() | pass()) >> formant(440.0))
            | (saw_hz(440.0) | pass() | pass()) >> formant(440.0))
            >> join::<U3>();

        assert_audio_unit_snapshot!(
            "formants_stack_join",
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
