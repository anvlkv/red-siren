use std::{
    marker::PhantomData,
    ops::{Add, Mul},
};

use fundsp::{
    numeric_array::ArrayLength,
    prelude::*,
    typenum::{Prod, Sum, Unsigned},
};

const ADSR_3D_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Adsr3D"));

/// One lerp segment: output on `channel`, interpolating from `start` to `end` over `duration_secs`.
#[derive(Clone)]
struct Segment {
    channel: usize,
    start: f32,
    end: f32,
    duration_secs: f32,
}

#[derive(Clone)]
struct TailFade {
    channel: usize,
    value: f32,
    phase: f32,
    duration_secs: f32,
}

/// One triggered cycle: walks through its Segments sequentially, one at a time.
/// Each segment's start is the previous segment's end (the fold).
#[derive(Clone)]
struct Cycle {
    segments: Vec<Segment>,
    seg_idx: usize,
    phase: f32,
    ingress_phase: f32,
    ingress_duration_secs: f32,
    tail_fade: Option<TailFade>,
}

impl Cycle {
    fn new(segments: Vec<Segment>, transition_ratio: f32, min_duration_secs: f32) -> Self {
        let ingress_duration_secs = segments
            .first()
            .map(|seg| (seg.duration_secs * transition_ratio).max(min_duration_secs))
            .unwrap_or(min_duration_secs);
        Self {
            segments,
            seg_idx: 0,
            phase: 0.0,
            ingress_phase: 0.0,
            ingress_duration_secs,
            tail_fade: None,
        }
    }

    fn is_done(&self) -> bool {
        self.seg_idx >= self.segments.len() && self.tail_fade.is_none()
    }

    fn ingress_gain(&self) -> f32 {
        smooth_ratio(self.ingress_phase)
    }

    /// Current output value for `channel`; 0 if this cycle is not currently on that channel.
    fn output_for(&self, channel: usize) -> f32 {
        let active = match self.segments.get(self.seg_idx) {
            Some(seg) if seg.channel == channel => {
                let t = smooth_ratio(self.phase);
                seg.start + (seg.end - seg.start) * t
            }
            _ => 0.0,
        };

        let tail = match &self.tail_fade {
            Some(tail) if tail.channel == channel => {
                let fade = 1.0 - smooth_ratio(tail.phase);
                tail.value * fade
            }
            _ => 0.0,
        };

        (active + tail) * self.ingress_gain()
    }

    fn start_tail_fade(
        &mut self,
        old_seg_idx: usize,
        min_duration_secs: f32,
        transition_ratio: f32,
    ) {
        let Some(old_seg) = self.segments.get(old_seg_idx) else {
            return;
        };
        let Some(next_seg) = self.segments.get(self.seg_idx) else {
            self.tail_fade = None;
            return;
        };
        if old_seg.channel == next_seg.channel {
            self.tail_fade = None;
            return;
        }

        self.tail_fade = Some(TailFade {
            channel: old_seg.channel,
            value: old_seg.end,
            phase: 0.0,
            duration_secs: (old_seg.duration_secs * transition_ratio).max(min_duration_secs),
        });
    }

    fn advance_smoothing(&mut self, sample_dt: f32) {
        if self.ingress_phase < 1.0 {
            let dur = self.ingress_duration_secs.max(f32::EPSILON);
            self.ingress_phase = (self.ingress_phase + sample_dt / dur).min(1.0);
        }

        let Some(tail) = &mut self.tail_fade else {
            return;
        };
        let dur = tail.duration_secs.max(f32::EPSILON);
        tail.phase = (tail.phase + sample_dt / dur).min(1.0);
        if tail.phase >= 1.0 {
            self.tail_fade = None;
        }
    }

    /// Advance by `sample_dt` seconds. Returns true while the cycle still has segments to run.
    fn advance(&mut self, sample_dt: f32, min_duration_secs: f32, transition_ratio: f32) -> bool {
        self.advance_smoothing(sample_dt);

        let mut remaining = sample_dt;
        let guard_max = self.segments.len() * 2 + 2;
        let mut guard = 0;
        while remaining > 0.0 && guard < guard_max {
            guard += 1;
            if self.seg_idx >= self.segments.len() {
                return false;
            }
            let dur = self.segments[self.seg_idx].duration_secs.max(f32::EPSILON);
            let to_end = (1.0 - self.phase).max(0.0);
            if to_end <= f32::EPSILON {
                let old_seg_idx = self.seg_idx;
                self.seg_idx += 1;
                self.phase = 0.0;
                self.start_tail_fade(old_seg_idx, min_duration_secs, transition_ratio);
                continue;
            }
            let time_to_end = to_end * dur;
            let consumed = remaining.min(time_to_end);
            self.phase = (self.phase + consumed / dur).min(1.0);
            remaining -= consumed;
            if self.phase >= 1.0 {
                let old_seg_idx = self.seg_idx;
                self.seg_idx += 1;
                self.phase = 0.0;
                self.start_tail_fade(old_seg_idx, min_duration_secs, transition_ratio);
            } else {
                break;
            }
        }
        !self.is_done()
    }
}

fn smooth_ratio(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
    cycles: Vec<Cycle>,
    prev_gate_high: bool,
}

impl<F: Real + 'static, P: Size<f32> + Unsigned + ArrayLength> Adsr3D<F, P> {
    const DEFAULT_DURATION_SECS: f32 = 0.1;
    const MIN_DURATION_SECS: f32 = 1.0e-4;
    const TRANSITION_RATIO: f32 = 0.15;

    pub fn new() -> Self {
        Self {
            _sample_type: PhantomData,
            _channels: PhantomData,
            sample_rate: DEFAULT_SR,
            cycles: Vec::new(),
            prev_gate_high: false,
        }
    }

    fn num_channels() -> usize {
        P::USIZE
    }

    fn read_duration(input: &[f32], ch: usize) -> f32 {
        let v = input[1 + ch * 2];
        if v.is_finite() && v > 0.0 {
            v.max(Self::MIN_DURATION_SECS)
        } else {
            Self::DEFAULT_DURATION_SECS
        }
    }

    fn read_weight(input: &[f32], ch: usize) -> f32 {
        let v = input[2 + ch * 2];
        if v.is_finite() {
            v
        } else {
            1.0
        }
    }

    /// Build a Cycle from current input values at trigger time.
    /// Each segment's start = previous segment's end (the fold).
    fn make_cycle(input: &[f32], gate: f32) -> Cycle {
        let n = P::USIZE;
        let mut segments = Vec::with_capacity(n);
        let mut prev_end = 0.0f32;
        for ch in 0..n {
            let end = gate * Self::read_weight(input, ch);
            segments.push(Segment {
                channel: ch,
                start: prev_end,
                end,
                duration_secs: Self::read_duration(input, ch),
            });
            prev_end = end;
        }
        Cycle::new(segments, Self::TRANSITION_RATIO, Self::MIN_DURATION_SECS)
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

    fn reset(&mut self) {
        self.cycles.clear();
        self.prev_gate_high = false;
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate.max(1.0);
        self.reset();
    }

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let input_slice = input.as_slice();
        let gate = if input[0].is_finite() { input[0] } else { 0.0 };
        let gate_high = gate > 0.0;

        if gate_high && !self.prev_gate_high {
            self.cycles.push(Self::make_cycle(input_slice, gate));
        }
        self.prev_gate_high = gate_high;

        let mut output = Frame::<f32, Self::Outputs>::default();
        let n = Self::num_channels();
        let sample_dt = 1.0 / self.sample_rate.max(1.0) as f32;

        for cycle in &self.cycles {
            for ch in 0..n {
                output[ch] += cycle.output_for(ch);
            }
        }

        self.cycles.retain_mut(|cycle| {
            cycle.advance(sample_dt, Self::MIN_DURATION_SECS, Self::TRANSITION_RATIO)
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
