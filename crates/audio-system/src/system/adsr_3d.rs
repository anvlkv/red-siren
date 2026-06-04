use std::{
    marker::PhantomData,
    ops::{Add, Mul, Sub},
};

use fundsp::{
    numeric_array::{ArrayLength, NumericArray},
    prelude::*,
    typenum::{Diff, Prod, Sum, Unsigned},
};

const ADSR_3D_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Adsr3D"));

#[derive(Clone, Copy)]
struct Segment<F: Real> {
    target: F,
    start: F,
    current: F,
    starts_in_samples: usize,
    duration_samples: usize,
    remaining_samples: usize,
}

impl<F: Real> Default for Segment<F> {
    fn default() -> Self {
        Self {
            target: F::zero(),
            start: F::zero(),
            current: F::zero(),
            starts_in_samples: 0,
            duration_samples: 0,
            remaining_samples: 0,
        }
    }
}

impl<F: Real> Segment<F> {
    fn duration_samples(duration: f64, sample_rate: f64) -> usize {
        (duration.max(0.0) * sample_rate).round() as usize
    }

    fn samples_duration(samples: usize, sample_rate: f64) -> f64 {
        samples as f64 / sample_rate.max(1.0)
    }

    fn progress(&self) -> F {
        let elapsed = self.duration_samples.saturating_sub(self.remaining_samples);
        if self.duration_samples <= 1 {
            F::one()
        } else {
            F::from_f64((elapsed + 1) as f64 / self.duration_samples as f64)
        }
    }

    fn smoother_step(t: F) -> F {
        let six = F::from_f32(6.0);
        let fifteen = F::from_f32(15.0);
        let ten = F::from_f32(10.0);
        t * t * t * (t * (t * six - fifteen) + ten)
    }

    fn advance(&mut self) -> Option<F> {
        if self.remaining_samples == 0 {
            return None;
        }
        if self.starts_in_samples > 0 {
            self.starts_in_samples -= 1;
            return None;
        }

        let t = self.progress();
        let d = self.target - self.start;
        let u = Self::smoother_step(t);

        self.current = self.start + d * u;
        self.remaining_samples -= 1;

        Some(self.current)
    }

    fn update_sample_rate(&mut self, old_sr: f64, new_sr: f64) {
        let old_duration = Self::samples_duration(self.duration_samples, old_sr);
        self.duration_samples = Self::duration_samples(old_duration, new_sr);
        let elapsed_samples = self.duration_samples - self.remaining_samples;
        self.remaining_samples = self.duration_samples.saturating_sub(elapsed_samples);
        self.starts_in_samples = Self::duration_samples(
            Self::samples_duration(self.starts_in_samples, old_sr),
            new_sr,
        );
    }
}

#[derive(Clone)]
struct Cycle<F: Real, P: Size<f32> + Unsigned + ArrayLength> {
    segments: NumericArray<Segment<F>, P>,
    transitions: NumericArray<(Option<Segment<F>>, Option<Segment<F>>), P>,
}

impl<F: Real, P: Size<f32> + Unsigned + ArrayLength> Default for Cycle<F, P> {
    fn default() -> Self {
        Self {
            segments: NumericArray::default(),
            transitions: NumericArray::default(),
        }
    }
}

impl<F: Real, P: Size<f32> + Unsigned + ArrayLength> Cycle<F, P> {
    const TRANSITION_RATIO: f64 = 1.0 / 3.0;

    fn new(
        durations: NumericArray<F, P>,
        weights: NumericArray<F, P>,
        excitement: F,
        sample_rate: f64,
    ) -> Self {
        let mut cycle = Self::default();
        let mut prev_end = F::zero();
        let mut prev_end_samples = 0;
        let mut it = cycle
            .segments
            .iter_mut()
            .zip(cycle.transitions.iter_mut())
            .enumerate()
            .peekable();

        while let Some((ch, (segment, (transition_in, transition_out)))) = it.next() {
            let main_segment_target = excitement * weights[ch];
            let duration = durations[ch];
            let duration_samples = Segment::<F>::duration_samples(convert(duration), sample_rate);

            let segment_start = prev_end_samples;
            let main_segment_end = segment_start + duration_samples;

            *segment = Segment {
                target: main_segment_target,
                start: prev_end,
                current: prev_end,
                starts_in_samples: segment_start,
                duration_samples,
                remaining_samples: duration_samples,
            };

            *transition_in = ch.checked_sub(1).map(|prev_ch| {
                let prev_duration = durations[prev_ch];
                let transition_duration = prev_duration * convert(Self::TRANSITION_RATIO);
                let transition_duration_samples =
                    Segment::<F>::duration_samples(convert(transition_duration), sample_rate);

                Segment {
                    target: prev_end,
                    start: F::zero(),
                    current: F::zero(),
                    starts_in_samples: segment_start.saturating_sub(transition_duration_samples),
                    duration_samples: transition_duration_samples,
                    remaining_samples: transition_duration_samples,
                }
            });

            *transition_out = it.peek().as_ref().map(|(next_ch, _)| {
                let next_duration = durations[*next_ch];
                let transition_duration = next_duration * convert(Self::TRANSITION_RATIO);
                let transition_duration_samples =
                    Segment::<F>::duration_samples(convert(transition_duration), sample_rate);

                Segment {
                    target: F::zero(),
                    start: main_segment_target,
                    current: main_segment_target,
                    starts_in_samples: main_segment_end,
                    duration_samples: transition_duration_samples,
                    remaining_samples: transition_duration_samples,
                }
            });

            prev_end = main_segment_target;
            prev_end_samples = main_segment_end;
        }

        cycle
    }

    fn update_sample_rate(&mut self, old_sr: f64, new_sr: f64) {
        self.segments
            .iter_mut()
            .for_each(|segment| segment.update_sample_rate(old_sr, new_sr));
        self.transitions
            .iter_mut()
            .for_each(|(transition_in, transition_out)| {
                if let Some(t) = transition_in {
                    t.update_sample_rate(old_sr, new_sr);
                }
                if let Some(t) = transition_out {
                    t.update_sample_rate(old_sr, new_sr);
                }
            });
    }

    fn advance(&mut self) -> Option<Frame<F, P>> {
        let mut output = Frame::<F, P>::default();
        let mut had_some = false;

        for (ch, (segment, (transition_in, transition_out))) in self
            .segments
            .iter_mut()
            .zip(self.transitions.iter_mut())
            .enumerate()
        {
            let seg_next = segment.advance();
            let trans_in_next = transition_in.as_mut().and_then(|t| t.advance());
            let trans_out_next = transition_out.as_mut().and_then(|t| t.advance());
            let sample = seg_next
                .or(trans_in_next)
                .or(trans_out_next)
                .unwrap_or(F::zero());
            output[ch] = sample;
            had_some = had_some
                || seg_next.is_some()
                || trans_in_next.is_some()
                || trans_out_next.is_some();
        }

        if had_some {
            Some(output)
        } else {
            None
        }
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
pub struct Adsr3D<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength>
where
    P: Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>,
{
    _sample_type: PhantomData<F>,
    _channels: PhantomData<P>,
    sample_rate: f64,
    cycles: Vec<Cycle<F, P>>,
}

impl<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength> Adsr3D<F, P>
where
    P: Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>,
{
    const DEFAULT_DURATION_SECS: f64 = 0.1;
    const MIN_DURATION_SECS: f64 = 1.0e-4;

    pub fn new() -> Self {
        Self {
            _sample_type: PhantomData,
            _channels: PhantomData,
            sample_rate: DEFAULT_SR,
            cycles: Vec::with_capacity(12),
        }
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
        let v = input[2 + ch * 2];
        if v.is_finite() {
            F::from_f32(v)
        } else {
            F::one()
        }
    }

    /// Build a Cycle from current input values at trigger time.
    /// Each segment's start = previous segment's end (the fold).
    fn make_cycle(input: &[f32], gate: F, sample_rate: f64) -> Cycle<F, P> {
        let durations = NumericArray::<F, P>::generate(|ch| Self::read_duration(input, ch));
        let weights = NumericArray::<F, P>::generate(|ch| Self::read_weight(input, ch));
        Cycle::new(durations, weights, gate, sample_rate)
    }
}

impl<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength> AudioNode for Adsr3D<F, P>
where
    P: Mul<U2>,
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
                cycle.update_sample_rate(old_sr, self.sample_rate);
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

        let mut output = Frame::<F, Self::Outputs>::default();

        self.cycles.retain_mut(|cycle| {
            let next_frame: Option<Frame<F, Self::Outputs>> = cycle.advance();

            output
                .iter_mut()
                .zip(
                    next_frame
                        .as_ref()
                        .unwrap_or(&Frame::<F, Self::Outputs>::default())
                        .iter(),
                )
                .for_each(|(out, &next)| {
                    *out += next;
                });

            next_frame.is_some()
        });

        Frame::generate(|ch| convert(output[ch]))
    }
}

pub fn adsr_3d<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength>() -> An<Adsr3D<F, P>>
where
    P: Mul<U2>,
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
    use insta_fun::prelude::*;

    fn interleaved_controls<P: Size<f32> + Unsigned + ArrayLength>(
        durations: Frame<f32, P>,
        weights: Frame<f32, P>,
    ) -> An<impl AudioNode<Inputs = U0, Outputs = Prod<P, U2>>>
    where
        P: Mul<U2> + Add<P>,
        Prod<P, U2>: Size<f32>,
        U1: Add<Prod<P, U2>>,
        Sum<U1, Prod<P, U2>>: Size<f32>,
        Sum<P, P>: Size<f32>,
    {
        let controls: An<Stack<Constant<P>, Constant<P>>> =
            constant::<Frame<f32, P>>(durations) | constant::<Frame<f32, P>>(weights);
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

    fn static_shape_adsr<P: Size<f32> + Unsigned + ArrayLength>(
        durations: Frame<f32, P>,
        weights: Frame<f32, P>,
    ) -> An<impl AudioNode<Inputs = U1, Outputs = P>>
    where
        P: Mul<U2> + Add<P>,
        Prod<P, U2>: Size<f32>,
        U1: Add<Prod<P, U2>>,
        Sum<U1, Prod<P, U2>>: Size<f32>,
        Sum<P, P>: Size<f32>,
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
                Frame::from([0.5, 0.5, 0.5,]),
                Frame::from([1.00, 0.25, 0.0]),
            ),
            single_impulse_gate(),
            SnapshotConfigBuilder::default()
                .sample_rate(2000.0)
                .num_samples(3000)
                .chart_layout(Layout::Combined)
                .build()
                .expect("snapshot config must be valid")
        );
    }

    #[test]
    fn adsr_3d_onsetting_impulse_static_shape_u4() {
        assert_audio_unit_snapshot!(
            "adsr_3d_onsetting_impulse_static_shape_u4",
            static_shape_adsr::<U4>(
                Frame::from([0.3, 0.3, 0.3, 0.3]),
                Frame::from([1.00, 0.30, 0.25, 0.0]),
            ),
            onsetting_impulse_gate(),
            SnapshotConfigBuilder::default()
                .sample_rate(2000.0)
                .num_samples(3000)
                .chart_layout(Layout::Combined)
                .build()
                .expect("snapshot config must be valid")
        );
    }

    #[test]
    fn adsr_3d_queued_retrigger_static_shape_u3() {
        assert_audio_unit_snapshot!(
            "adsr_3d_queued_retrigger_static_shape_u3",
            static_shape_adsr::<U3>(
                Frame::from([0.5, 0.5, 0.5,]),
                Frame::from([1.00, 0.25, 0.0]),
            ),
            queued_retrigger_gate(),
            SnapshotConfigBuilder::default()
                .sample_rate(2000.0)
                .num_samples(3000)
                .chart_layout(Layout::Combined)
                .build()
                .expect("snapshot config must be valid")
        );
    }
}
