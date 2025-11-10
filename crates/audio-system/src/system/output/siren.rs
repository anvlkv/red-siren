use std::{f32, f64};

#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::{hash_str, S};

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));

/// Siren oscillator with excitement-controlled pauses and frequency.
/// - Input 0: excitement level
/// - Input 1: base_hz
/// - Input 2: max_frequency_hz
/// - Input 3: excitement_pause_limit
/// - Input 4: base_pause_duration
/// - Output 0: siren wave with pauses and frequency.
#[derive(Default, Clone)]
pub struct Siren<F: Real> {
    phase: F,
    initial_phase: Option<F>,
    sign: F,
    sample_duration: F,
    hash: u64,
}

impl<F: Real> Siren<F> {
    /// Create siren oscillator.
    pub fn new() -> Self {
        let mut siren = Self::default();
        siren.reset();
        siren.set_sample_rate(DEFAULT_SR);
        siren
    }

    #[allow(dead_code)]
    pub fn new_phase(phase: F) -> Self {
        let mut siren = Self {
            initial_phase: Some(phase),
            ..Self::default()
        };
        siren.reset();
        siren.set_sample_rate(DEFAULT_SR);
        siren
    }

    /// wave shape fn
    ///
    /// - time: current time
    /// - alpha: full period
    /// - beta: slow decay
    /// - gamma: sharp onset
    fn shape(time: F, alpha: F, beta: F, gamma: F) -> F {
        #[cfg(feature = "hi_fi")]
        let e: F = F::from_f64(f64::consts::E);
        #[cfg(not(feature = "hi_fi"))]
        let e: F = F::from_f32(f32::consts::E);

        #[cfg(feature = "hi_fi")]
        let pi: F = F::from_f64(f64::consts::PI);
        #[cfg(not(feature = "hi_fi"))]
        let pi: F = F::from_f32(f32::consts::PI);

        (e.pow(-time / (beta * alpha)) - e.pow(-time / (gamma * alpha)))
            * cos((pi * time) / alpha).pow(convert(2.0))
    }

    /// returns `(sample, next_phase, next_sign)`
    fn tick_internal(
        excitement: F,
        alpha: F,
        sample_duration: F,
        mut phase: F,
        mut sign: F,
        beta: F,
        gamma: F,
    ) -> (F, F, F) {
        let wrap_phase: F = alpha + alpha * gamma + alpha * beta;

        let next_phase = phase + sample_duration;

        if excitement == F::zero()
            && (phase == F::zero() || phase == F::zero() || next_phase >= wrap_phase)
        {
            (convert(0.0), phase, sign)
        } else {
            phase = if next_phase >= wrap_phase {
                sign = -sign;
                next_phase - wrap_phase
            } else {
                next_phase
            };

            let non_zero_excitement = excitement.max(convert(S::EPSILON));

            let sample = Self::shape(
                phase,
                alpha * non_zero_excitement,
                beta / excitement,
                gamma / excitement,
            ) * sign;

            (sample, phase, sign)
        }
    }
}

impl<F: Real> AudioNode for Siren<F> {
    const ID: u64 = SIREN_ID;
    type Inputs = typenum::U4;
    type Outputs = typenum::U1;

    fn reset(&mut self) {
        self.phase = match self.initial_phase {
            Some(phase) => phase,
            None => convert(rnd1(self.hash)),
        };
        self.sign = convert(1.0);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_duration = convert(1.0 / sample_rate);
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let excitement: F = convert(input[0] as S);
        let alpha: F = convert(input[1] as S);
        let beta: F = convert(input[2] as S);
        let gamma: F = convert(input[3] as S);

        let (sample, next_phase, next_sign) = Self::tick_internal(
            excitement,
            alpha,
            self.sample_duration,
            self.phase,
            self.sign,
            beta,
            gamma,
        );
        self.phase = next_phase;
        self.sign = next_sign;

        [sample.to_f32()].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        let mut phase = self.phase;
        let mut sign = self.sign;

        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let idx = (i << SIMD_S) + j;

                let excitement = F::from_f32(input.at_f32(0, idx));
                let alpha = F::from_f32(input.at_f32(1, idx));
                let beta = F::from_f32(input.at_f32(2, idx));
                let gamma = F::from_f32(input.at_f32(3, idx));

                let (sample, next_phase, next_sign) = Self::tick_internal(
                    excitement,
                    alpha,
                    self.sample_duration,
                    phase,
                    sign,
                    beta,
                    gamma,
                );

                phase = next_phase;
                sign = next_sign;

                sample.to_f32()
            });
            output.set(0, i, F32x::new(element));
        }

        self.phase = phase;
        self.sign = sign;
        self.process_remainder(size, input, output);
    }

    fn set_hash(&mut self, hash: u64) {
        self.hash = hash;
        self.reset();
    }
}

pub fn siren<F>() -> An<Siren<F>>
where
    F: Real,
{
    let siren = Siren::new();
    An(siren)
}

#[allow(dead_code)]
pub fn siren_phase<F>(phase: F) -> An<Siren<F>>
where
    F: Real,
{
    let siren = Siren::new_phase(phase);
    An(siren)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    const ALPHA: f32 = 1.0 / 7500.0;
    const BETA: f32 = 0.75;
    const GAMMA: f32 = 0.3;
    const SAMPLES: usize = 2250;

    const fn excitement(sample: usize) -> f32 {
        match sample {
            ..50 => 0.0,
            50..250 => f32::EPSILON,
            250..500 => 0.01,
            500..750 => 0.1,
            750..1000 => 0.25,
            1000..1250 => 0.3,
            1250..1500 => 0.5,
            1500..1750 => 0.75,
            1750..2000 => 0.99,
            _ => 1.0,
        }
    }

    #[test]
    fn test_siren_tick() {
        let siren_node = siren_phase::<f32>(0.0);
        let config = SnapshotConfigBuilder::default()
            .num_samples(SAMPLES)
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(
            "siren_0001",
            siren_node.clone(),
            InputSource::Generator(Box::new(|sample, ch| {
                match ch {
                    0 => excitement(sample),
                    1 => ALPHA,
                    2 => BETA,
                    3 => GAMMA,
                    _ => 0.0,
                }
            })),
            config.clone()
        );

        assert_audio_unit_snapshot!(
            "siren_0010",
            siren_node.clone(),
            InputSource::Generator(Box::new(|sample, ch| {
                match ch {
                    0 => excitement(sample),
                    1 => ALPHA * 10.0,
                    2 => BETA,
                    3 => GAMMA,
                    _ => 0.0,
                }
            })),
            config.clone()
        );

        assert_audio_unit_snapshot!(
            "siren_0070",
            siren_node.clone(),
            InputSource::Generator(Box::new(|sample, ch| {
                match ch {
                    0 => excitement(sample),
                    1 => ALPHA * 70.0,
                    2 => BETA,
                    3 => GAMMA,
                    _ => 0.0,
                }
            })),
            config.clone()
        );

        assert_audio_unit_snapshot!(
            "siren_3",
            siren_node,
            InputSource::Generator(Box::new(|sample, ch| {
                match ch {
                    0 => excitement(sample),
                    1 => 3.0,
                    2 => BETA,
                    3 => GAMMA,
                    _ => 0.0,
                }
            })),
            config
        );
    }

    #[test]
    fn test_siren_process() {
        let siren_node = siren_phase::<f32>(0.0);
        let config = SnapshotConfigBuilder::default()
            .num_samples(SAMPLES)
            .processing_mode(Processing::Batch(64))
            .build()
            .unwrap();

        let input = vec![0.7, ALPHA, BETA, GAMMA];

        assert_audio_unit_snapshot!(
            "siren_process_0_7",
            siren_node.clone(),
            InputSource::Flat(input),
            config.clone()
        );

        let input = vec![0.1, ALPHA, BETA, GAMMA];

        assert_audio_unit_snapshot!(
            "siren_process_0_1",
            siren_node,
            InputSource::Flat(input),
            config
        );
    }
}
