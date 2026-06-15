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

#[derive(Clone)]
struct CyclePoU<F: Real, P: Size<f32> + Unsigned + ArrayLength> {
    weights: NumericArray<F, P>,
    durations_secs: NumericArray<F, P>,
    starts_in_samples: NumericArray<usize, P>,
    duration_samples: NumericArray<usize, P>,
    boundary_overlaps: Vec<usize>,
    total_samples: usize,
    cursor_samples: usize,
    gate: F,
}

impl<F: Real, P: Size<f32> + Unsigned + ArrayLength> CyclePoU<F, P> {
    const TRANSITION_RATIO: f64 = 1.0 / 3.0;

    fn duration_samples(duration: f64, sample_rate: f64) -> usize {
        (duration.max(0.0) * sample_rate).round() as usize
    }

    fn rebuild_timeline(&mut self, sample_rate: f64) {
        let mut start = 0usize;
        for ch in 0..P::USIZE {
            let secs: f64 = convert(self.durations_secs[ch]);
            let seg_samples = Self::duration_samples(secs, sample_rate);
            self.starts_in_samples[ch] = start;
            self.duration_samples[ch] = seg_samples;
            start += seg_samples;
        }
        self.total_samples = start;

        self.boundary_overlaps.clear();
        self.boundary_overlaps.reserve(P::USIZE.saturating_sub(1));
        for b in 0..P::USIZE.saturating_sub(1) {
            let left_secs: f64 = convert(self.durations_secs[b]);
            let right_secs: f64 = convert(self.durations_secs[b + 1]);
            let overlap_secs = left_secs.min(right_secs) * Self::TRANSITION_RATIO;
            let overlap_samples = Self::duration_samples(overlap_secs, sample_rate);
            self.boundary_overlaps.push(overlap_samples);
        }
    }

    fn new(
        durations: NumericArray<F, P>,
        weights: NumericArray<F, P>,
        gate: F,
        sample_rate: f64,
    ) -> Self {
        let mut cycle = Self {
            weights,
            durations_secs: durations,
            starts_in_samples: NumericArray::default(),
            duration_samples: NumericArray::default(),
            boundary_overlaps: Vec::with_capacity(P::USIZE.saturating_sub(1)),
            total_samples: 0,
            cursor_samples: 0,
            gate,
        };
        cycle.rebuild_timeline(sample_rate);
        cycle
    }

    fn norm_u(pos: usize, len: usize) -> F {
        if len <= 1 {
            F::one()
        } else {
            F::from_f64((pos as f64 / (len - 1) as f64).clamp(0.0, 1.0))
        }
    }

    fn smoother_step(t: F) -> F {
        let six = F::from_f32(6.0);
        let fifteen = F::from_f32(15.0);
        let ten = F::from_f32(10.0);
        t * t * t * (t * (t * six - fifteen) + ten)
    }

    fn update_sample_rate(&mut self, old_sr: f64, new_sr: f64) {
        let elapsed_secs = self.cursor_samples as f64 / old_sr.max(1.0);
        self.rebuild_timeline(new_sr);
        self.cursor_samples = std::cmp::min(
            Self::duration_samples(elapsed_secs, new_sr),
            self.total_samples,
        );
    }

    fn shape_at(&self, n: usize) -> Frame<F, P> {
        let mut out = Frame::<F, P>::default();

        let segment_value = |ch: usize, sample: usize| -> F {
            let start = self.starts_in_samples[ch];
            let len = self.duration_samples[ch];
            if len == 0 {
                return self.weights[ch];
            }

            let local = sample.saturating_sub(start);
            let u = Self::norm_u(std::cmp::min(local, len.saturating_sub(1)), len);
            let shaped = Self::smoother_step(u);

            let from = if ch == 0 {
                F::zero()
            } else {
                self.weights[ch - 1]
            };
            let to = self.weights[ch];

            from + (to - from) * shaped
        };

        for boundary in 0..self.boundary_overlaps.len() {
            let overlap = self.boundary_overlaps[boundary];
            if overlap == 0 {
                continue;
            }

            let boundary_sample =
                self.starts_in_samples[boundary] + self.duration_samples[boundary];
            let window_start = boundary_sample.saturating_sub(overlap / 2);
            let window_end = window_start + overlap;

            if n >= window_start && n < window_end {
                let u = Self::norm_u(n - window_start, overlap);
                let phi = Self::smoother_step(u);
                let one = F::one();

                let left = segment_value(boundary, n);
                let right = segment_value(boundary + 1, n);
                out[boundary] = left * (one - phi);
                out[boundary + 1] = right * phi;
                return out;
            }
        }

        for ch in 0..P::USIZE {
            let start = self.starts_in_samples[ch];
            let end = start + self.duration_samples[ch];
            if n >= start && n < end {
                out[ch] = segment_value(ch, n);
                return out;
            }
        }

        out
    }

    fn advance(&mut self) -> Option<Frame<F, P>> {
        if self.cursor_samples >= self.total_samples {
            return None;
        }

        let shape = self.shape_at(self.cursor_samples);
        self.cursor_samples += 1;

        Some(Frame::generate(|ch| self.gate * shape[ch]))
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
    cycles: Vec<CyclePoU<F, P>>,
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
    fn make_cycle(input: &[f32], gate: F, sample_rate: f64) -> CyclePoU<F, P> {
        let durations = NumericArray::<F, P>::generate(|ch| Self::read_duration(input, ch));
        let weights = NumericArray::<F, P>::generate(|ch| Self::read_weight(input, ch));
        CyclePoU::new(durations, weights, gate, sample_rate)
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

pub fn create_adsr_3d<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength>() -> An<Adsr3D<F, P>>
where
    P: Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>,
{
    An(Adsr3D::new())
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
        (pass() | interleaved_controls(durations, weights)) >> create_adsr_3d::<f32, P>()
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
