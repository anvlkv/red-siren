use std::{
    marker::PhantomData,
    ops::{Add, Mul},
};

use fundsp::{
    numeric_array::{ArrayLength, NumericArray},
    prelude::*,
    typenum::{Prod, Sum, Unsigned},
};

const ADSR_3D_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Adsr3D"));

#[derive(Clone, Copy)]
struct Segment<F: Real> {
    target: F,
    current: F,
    starts_in_samples: usize,
    duration_samples: usize,
    remaining_samples: usize,
}

impl<F: Real> Default for Segment<F> {
    fn default() -> Self {
        Self {
            target: F::zero(),
            current: F::zero(),
            starts_in_samples: 0,
            duration_samples: 0,
            remaining_samples: 0,
        }
    }
}

impl<F: Real> Segment<F> {
    fn started(&self) -> bool {
        self.starts_in_samples == 0
    }

    fn finished(&self) -> bool {
        self.remaining_samples == 0
    }

    fn duration_samples(duration: f64, sample_rate: f64) -> usize {
        (duration.max(0.0) * sample_rate).round() as usize
    }

    fn samples_duration(samples: usize, sample_rate: f64) -> f64 {
        samples as f64 / sample_rate.max(1.0)
    }

    fn progress(&self) -> F {
        if self.duration_samples == 0 {
            F::one()
        } else {
            F::one()
                - F::from_f64(self.remaining_samples as f64)
                    / F::from_f64(self.duration_samples as f64)
        }
    }

    fn easing(&self) -> F {
        let t = self.progress();
        t.pow(F::from_f32(3.0))
    }

    fn advance(&mut self) -> Option<F> {
        if self.remaining_samples == 0 {
            return None;
        }
        if self.starts_in_samples > 0 {
            self.starts_in_samples -= 1;
            return None;
        }

        let increment = ((self.target - self.current) / F::from_f64(self.remaining_samples as f64))
            * self.easing();

        self.current += increment;
        self.remaining_samples -= 1;

        Some(self.current)
    }
}

#[derive(Clone)]
/// Rotates gradually between P channels.
///
/// Inputs: [gate, dur_0, weight_0, dur_1, weight_1, ..., dur_P-1, weight_P-1]
/// Outputs: [out_0, out_1, ..., out_P-1]
///
/// Each gate rising edge launches a new Cycle: P sequential segments where segment i
/// lerps from `gate * weight[i-1]` (0 for i=0) to `gate * weight[i]` over `dur[i]`.
/// Multiple concurrent cycles (onsetting) sum their outputs per channel.
pub struct Adsr3D<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength> {
    _sample_type: PhantomData<F>,
    _channels: PhantomData<P>,
    sample_rate: f64,
    cycles: Vec<NumericArray<Segment<F>, P>>,
}

impl<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength> Adsr3D<F, P> {
    const DEFAULT_DURATION_SECS: f64 = 0.1;
    const MIN_DURATION_SECS: f64 = 1.0e-4;
    const TRANSITION_RATIO: f64 = 1.0 / 3.0;

    pub fn new() -> Self {
        Self {
            _sample_type: PhantomData,
            _channels: PhantomData,
            sample_rate: DEFAULT_SR,
            cycles: Vec::new(),
        }
    }

    fn num_channels() -> usize {
        P::USIZE
    }

    fn read_duration(input: &[f32], ch: usize) -> F {
        let v = input[1 + ch * 2] as f64;
        if v.is_finite() && v > 0.0 {
            F::from_f64(v.max(Self::MIN_DURATION_SECS))
        } else {
            F::from_f64(Self::DEFAULT_DURATION_SECS)
        }
    }

    fn read_weight(input: &[f32], ch: usize) -> F {
        let v = input[2 + ch * 2] as f64;
        if v.is_finite() {
            F::from_f64(v)
        } else {
            F::one()
        }
    }

    /// Build a Cycle from current input values at trigger time.
    /// Each segment's start = previous segment's end (the fold).
    fn make_cycle(input: &[f32], gate: F, sample_rate: f64) -> NumericArray<Segment<F>, P> {
        let mut cycle = NumericArray::<Segment<F>, P>::default();
        // let n = P::USIZE;
        // let mut segments = Vec::with_capacity(n);
        let mut prev_end = F::zero();
        let mut prev_end_samples = 0;
        for (ch, weight) in cycle.iter_mut().enumerate() {
            let input_weight = Self::read_weight(input, ch);
            let duration = Self::read_duration(input, ch);
            let target = gate * input_weight;
            let duration_samples = Segment::<F>::duration_samples(convert(duration), sample_rate);
            *weight = Segment {
                target,
                current: prev_end,
                starts_in_samples: prev_end_samples,
                duration_samples,
                remaining_samples: duration_samples,
            };
            prev_end = target;
            prev_end_samples += duration_samples
        }
        cycle
    }

    fn transition_segments(
        prev: Option<&Segment<F>>,
        current: &Segment<F>,
        next: Option<&Segment<F>>,
    ) -> F {
        if current.finished() {
            return F::zero();
        }

        if !current.started()
            && prev.is_none_or(|p| !p.started())
            && next.is_none_or(|n| !n.started())
        {
            return F::zero();
        }

        todo!()
    }
}

impl<F: Real + 'static, P> AudioNode for Adsr3D<F, P>
where
    P: Size<f32> + Unsigned + ArrayLength + Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>,
{
    const ID: u64 = ADSR_3D_ID;

    type Inputs = Sum<U1, Prod<P, U2>>;

    type Outputs = P;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let old_sr = self.sample_rate;
        self.sample_rate = sample_rate.max(1.0);
        if self.sample_rate != old_sr {
            for cycle in self.cycles.iter_mut() {
                for segment in cycle.iter_mut() {
                    segment.duration_samples = Segment::<F>::duration_samples(
                        Segment::<F>::samples_duration(segment.duration_samples, old_sr),
                        sample_rate,
                    );
                    segment.remaining_samples = Segment::<F>::duration_samples(
                        Segment::<F>::samples_duration(segment.remaining_samples, old_sr),
                        sample_rate,
                    );
                    segment.starts_in_samples = Segment::<F>::duration_samples(
                        Segment::<F>::samples_duration(segment.starts_in_samples, old_sr),
                        sample_rate,
                    );
                }
            }
        }
    }

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let input_slice = input.as_slice();
        let gate = if input[0].is_finite() { input[0] } else { 0.0 };

        if gate > 0.0 {
            self.cycles.push(Self::make_cycle(
                input_slice,
                convert(gate),
                self.sample_rate,
            ));
        }

        let mut output = Frame::<f32, Self::Outputs>::default();

        self.cycles.retain_mut(|cycle| {
            let mut had_some = false;
            let mut it = cycle.iter_mut().enumerate().peekable();
            let mut prev = Option::<Segment<F>>::None;

            while let Some((ch, segment)) = it.next() {
                let seg_next = segment.advance();

                had_some = seg_next.is_some() || had_some;
                output[ch] += convert::<F, f32>(seg_next.unwrap_or_else(|| {
                    let next = it.peek().map(|(_, s)| **s);
                    Self::transition_segments(prev.as_ref(), segment, next.as_ref())
                }));
                prev = Some(*segment);
            }

            had_some
        });
        output
    }
}

pub fn adsr_3d<F: Real + 'static, P>() -> An<Adsr3D<F, P>>
where
    P: Size<f32> + Unsigned + ArrayLength + Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>,
{
    An(Adsr3D::new())
    // ^ type inference needs explicit turbofish when P is no longer in a concrete field
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::low_sr_snapshot_collate;
    use insta_fun::prelude::*;

    const SNAP_LEN: usize = 240;

    fn interleaved_controls<P>(
        durations: Frame<f32, P>,
        weights: Frame<f32, P>,
    ) -> An<impl AudioNode<Inputs = U0, Outputs = Prod<P, U2>>>
    where
        P: Size<f32> + Unsigned + ArrayLength + Add<P> + Mul<U2>,
        Sum<P, P>: Size<f32>,
        Prod<P, U2>: Size<f32>,
    {
        let controls = constant::<Frame<f32, P>>(durations) | constant::<Frame<f32, P>>(weights);
        controls
            >> map(|frame: &Frame<f32, Sum<P, P>>| -> Frame<f32, Prod<P, U2>> {
                Frame::generate(|i| {
                    let phase = i / 2;
                    if i % 2 == 0 {
                        frame[phase]
                    } else {
                        frame[P::USIZE + phase]
                    }
                })
            })
    }

    fn static_shape_adsr<P>(
        durations: Frame<f32, P>,
        weights: Frame<f32, P>,
    ) -> An<impl AudioNode<Inputs = U1, Outputs = P>>
    where
        P: Size<f32> + Unsigned + ArrayLength + Add<P> + Mul<U2>,
        Sum<P, P>: Size<f32>,
        Prod<P, U2>: Size<f32>,
        U1: Add<Prod<P, U2>>,
        Sum<U1, Prod<P, U2>>: Size<f32>,
    {
        (pass() | interleaved_controls(durations, weights)) >> adsr_3d::<f32, P>()
    }

    fn single_impulse_gate() -> InputSource {
        InputSource::Generator(Box::new(
            |i, channel| {
                if channel == 0 && i == 0 {
                    1.0
                } else {
                    0.0
                }
            },
        ))
    }

    fn onsetting_impulse_gate() -> InputSource {
        InputSource::Generator(Box::new(|i, channel| {
            if channel == 0 && (i == 0 || i == 40) {
                1.0
            } else {
                0.0
            }
        }))
    }

    fn queued_retrigger_gate() -> InputSource {
        InputSource::Generator(Box::new(|i, channel| {
            if channel == 0 && (i == 0 || i == 20) {
                1.0
            } else {
                0.0
            }
        }))
    }

    #[test]
    fn adsr_3d_single_impulse_static_shape_u3() {
        assert_audio_unit_snapshot!(
            "adsr_3d_single_impulse_static_shape_u3",
            static_shape_adsr::<U3>(
                Frame::from([0.10, 0.18, 0.24]),
                Frame::from([1.00, 0.35, 0.0]),
            ),
            single_impulse_gate(),
            low_sr_snapshot_collate(SNAP_LEN)
        );
    }

    #[test]
    fn adsr_3d_onsetting_impulse_static_shape_u4() {
        assert_audio_unit_snapshot!(
            "adsr_3d_onsetting_impulse_static_shape_u4",
            static_shape_adsr::<U4>(
                Frame::from([0.10, 0.12, 0.22, 0.20]),
                Frame::from([1.00, 0.30, 0.25, 0.0]),
            ),
            onsetting_impulse_gate(),
            low_sr_snapshot_collate(SNAP_LEN)
        );
    }

    #[test]
    fn adsr_3d_queued_retrigger_static_shape_u3() {
        assert_audio_unit_snapshot!(
            "adsr_3d_queued_retrigger_static_shape_u3",
            static_shape_adsr::<U3>(
                Frame::from([0.10, 0.18, 0.24]),
                Frame::from([1.00, 0.35, 0.0]),
            ),
            queued_retrigger_gate(),
            low_sr_snapshot_collate(SNAP_LEN)
        );
    }
}
