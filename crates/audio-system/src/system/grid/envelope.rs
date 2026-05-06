use std::{marker::PhantomData, ops::Add};

use fundsp::{
    numeric_array::ArrayLength,
    prelude::*,
    typenum::{UInt, UTerm, B0, B1},
};
use num_rational::Ratio;
use typenum::op;

use crate::grid::AdsrShape;

use super::rhythm::RATIO_SCALE;

const RHYTHM_GRID_ENVELOPE_ID: u64 =
    crate::util::hash_str(concat!(module_path!(), "::RhythmGridEnvelope"));

#[derive(Clone)]
struct Schedule<S: Real + Float + 'static> {
    start_ticks: u64,
    attack: u64,
    decay: u64,
    sustain: S,
    release: u64,
    total_duration: u64,
    smoothness: S,
    decay_exponent: S,
}

#[derive(Clone)]
struct ActiveState<S: Real + Float + 'static> {
    ticks_until_start: u64,
    envelope_tick: u64,
    schedule: Schedule<S>,
}

#[derive(Clone)]
/// An envelope generator that produces an ADSR envelope synchronized to the rhythm grid.
///
/// ## Inputs: `U3 + U1 + U1 + NI`
///
/// Each tick where start/duration inputs are finite and positive, a new schedule is enqueued.
/// The caller is responsible for pulsing inputs for exactly one tick per desired event.
/// The grid trigger pops one pending schedule and activates it — on the current beat if
/// the trigger and pulse arrive on the same tick, otherwise on the next beat trigger.
/// Each schedule fires exactly once; to repeat, pulse the inputs again.
///
/// ### Grid inputs: `U3`
/// - Grid trigger signal (1.0 on the first tick of each beat, 0.0 otherwise)
/// - Current ticks per beat as a float
/// - Current ticks to next beat as a float (unused — reserved for future use)
///
/// ### Scheduler start input: `U1`
/// - Start beat ratio (e.g. 0.25 for quarter-beat offset)
///
/// ### Scheduler duration input: `U1`
/// - Duration beat ratio (e.g. 0.5 for half-beat, 1.5 for dotted beat)
///
/// ### Inner node inputs: `NI`
///
/// ## Outputs: `N::Outputs`
pub struct RhythmGridEnvelope<
    S: Real + Float + 'static,
    N: AudioNode<Inputs = NI>,
    NI: Size<S> + Size<N>,
> {
    inner: An<N>,
    adsr: AdsrShape<S>,
    pending_schedules: Vec<Schedule<S>>,
    active: Vec<ActiveState<S>>,
    _sample_type: PhantomData<S>,
    _inner_inputs: PhantomData<NI>,
}

impl<S: Real + Float + 'static, N: AudioNode<Inputs = NI>, NI: Size<S> + Size<N>>
    RhythmGridEnvelope<S, N, NI>
where
    UInt<UInt<UInt<UTerm, B1>, B0>, B1>: Add<NI>,
    <UInt<UInt<UInt<UTerm, B1>, B0>, B1> as Add<NI>>::Output: ArrayLength + Send + Sync,
{
    pub fn new(inner: An<N>, adsr: AdsrShape<S>) -> Self {
        Self {
            inner,
            adsr,
            pending_schedules: Vec::new(),
            active: Vec::new(),
            _sample_type: PhantomData,
            _inner_inputs: PhantomData,
        }
    }

    fn as_i64(value: u64) -> i64 {
        i64::try_from(value).unwrap_or(i64::MAX)
    }

    fn ratio_floor_u64(value: &Ratio<i64>) -> u64 {
        let floored = value.floor().to_integer();
        if floored <= 0 {
            0
        } else {
            floored as u64
        }
    }

    fn ratio_ceil_u64(value: &Ratio<i64>) -> u64 {
        let ceiled = value.ceil().to_integer();
        if ceiled <= 0 {
            0
        } else {
            ceiled as u64
        }
    }

    fn unit_ratio(value: S) -> Ratio<i64> {
        let raw = value.to_f64();
        if !raw.is_finite() || raw <= 0.0 {
            return Ratio::from_integer(0);
        }
        let clamped = raw.min(1.0);
        let scaled = (clamped * RATIO_SCALE as f64).round() as i64;
        Ratio::new(scaled, RATIO_SCALE)
    }

    fn nonneg_ratio(value: S) -> Ratio<i64> {
        let raw = value.to_f64();
        if !raw.is_finite() || raw <= 0.0 {
            return Ratio::from_integer(0);
        }
        let scaled = (raw * RATIO_SCALE as f64).round() as i64;
        Ratio::new(std::cmp::Ord::max(scaled, 0), RATIO_SCALE)
    }

    /// Returns a beat-ratio for duration inputs: accepts only positive, finite values.
    /// `NaN` or `<= 0.0` are treated as "no schedule" and return `None`.
    fn positive_beat_ratio(value: f32) -> Option<Ratio<i64>> {
        if !value.is_finite() || value <= 0.0 {
            return None;
        }
        let scaled = (value as f64 * RATIO_SCALE as f64).round() as i64;
        if scaled <= 0 {
            return None;
        }
        Some(Ratio::new(scaled, RATIO_SCALE))
    }

    /// Returns a beat-ratio for start inputs: accepts `0.0` (immediate) and positive finite
    /// values. `NaN` or negative values are treated as "no schedule" and return `None`.
    fn nonneg_beat_ratio(value: f32) -> Option<Ratio<i64>> {
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        let scaled = (value as f64 * RATIO_SCALE as f64).round() as i64;
        Some(Ratio::new(std::cmp::Ord::max(scaled, 0), RATIO_SCALE))
    }

    fn compute_schedule(
        adsr: &AdsrShape<S>,
        ticks_per_beat: u64,
        start_ratio_beats: Ratio<i64>,
        duration_ratio_beats: Ratio<i64>,
    ) -> Schedule<S> {
        let ticks_per_beat_ratio = Ratio::from_integer(Self::as_i64(ticks_per_beat));
        let start_ratio = ticks_per_beat_ratio * start_ratio_beats;
        let duration_ratio = ticks_per_beat_ratio * duration_ratio_beats;

        // Explicit policy: start uses floor; duration uses ceil and remains at least one tick.
        let start_ticks = Self::ratio_floor_u64(&start_ratio);
        let total_ticks = Ord::max(Self::ratio_ceil_u64(&duration_ratio), 1);
        let total_ticks_ratio = Ratio::from_integer(Self::as_i64(total_ticks));

        // Explicit policy: stage lengths use floor and remain at least one tick.
        let attack = Ord::max(
            Self::ratio_floor_u64(&(total_ticks_ratio * Self::unit_ratio(adsr.attack))),
            1,
        );
        let decay = Ord::max(
            Self::ratio_floor_u64(&(total_ticks_ratio * Self::nonneg_ratio(adsr.decay))),
            1,
        );
        let release = Ord::max(
            Self::ratio_floor_u64(&(total_ticks_ratio * Self::unit_ratio(adsr.release))),
            1,
        );
        Schedule {
            start_ticks,
            attack,
            decay,
            sustain: adsr.sustain,
            release,
            total_duration: total_ticks,
            smoothness: adsr.smoothness,
            decay_exponent: adsr.decay_exponent,
        }
    }

    fn envelope_value(tick: u64, schedule: &Schedule<S>) -> S {
        let zero = S::zero();
        let one = S::one();

        if tick >= schedule.total_duration {
            return zero;
        }

        // A one-tick schedule must produce an audible value on its only sample.
        if schedule.total_duration == 1 {
            return one;
        }
        let attack_end = schedule.attack;
        let decay_end = attack_end + schedule.decay;
        let release_start = schedule.total_duration.saturating_sub(schedule.release);
        if tick < attack_end {
            // attack: 0 → 1
            let t = S::from_f64(tick as f64 / schedule.attack as f64);
            AdsrShape::<S>::smooth_progress(t, schedule.smoothness)
        } else if tick < decay_end {
            // decay: 1 → sustain
            let t = S::from_f64((tick - attack_end) as f64 / schedule.decay as f64);
            let p = AdsrShape::<S>::tail_progress(t, schedule.smoothness, schedule.decay_exponent);
            one - p * (one - schedule.sustain)
        } else if tick < release_start {
            // sustain
            schedule.sustain
        } else {
            // release: sustain → 0
            let t = S::from_f64((tick - release_start) as f64 / schedule.release as f64);
            let p = AdsrShape::<S>::tail_progress(t, schedule.smoothness, schedule.decay_exponent);
            schedule.sustain * (one - p)
        }
    }

    fn step_envelope(
        &mut self,
        grid_trigger: f32,
        ticks_per_beat: u64,
        start_ratio_beats: f32,
        duration_ratio_beats: f32,
    ) -> S {
        if ticks_per_beat > 0 {
            if let (Some(start_ratio), Some(duration_ratio)) = (
                Self::nonneg_beat_ratio(start_ratio_beats),
                Self::positive_beat_ratio(duration_ratio_beats),
            ) {
                let schedule =
                    Self::compute_schedule(&self.adsr, ticks_per_beat, start_ratio, duration_ratio);
                self.pending_schedules.push(schedule);
            }
        }

        if grid_trigger == 1.0 && !self.pending_schedules.is_empty() {
            let schedule = self.pending_schedules.remove(0);
            self.active.push(ActiveState {
                ticks_until_start: schedule.start_ticks,
                envelope_tick: 0,
                schedule,
            });
        }

        let mut env_value = S::zero();
        for active in &mut self.active {
            if active.ticks_until_start > 0 {
                active.ticks_until_start -= 1;
            } else {
                let val = Self::envelope_value(active.envelope_tick, &active.schedule);
                active.envelope_tick += 1;
                env_value += val;
            }
        }

        self.active.retain(|a| {
            if a.ticks_until_start > 0 {
                true
            } else {
                a.envelope_tick < a.schedule.total_duration
            }
        });

        env_value
    }
}

impl<S: Real + Float, N: AudioNode<Inputs = NI>, NI: Size<S> + Size<N>> AudioNode
    for RhythmGridEnvelope<S, N, NI>
where
    UInt<UInt<UInt<UTerm, B1>, B0>, B1>: Add<NI>,
    <UInt<UInt<UInt<UTerm, B1>, B0>, B1> as Add<NI>>::Output: ArrayLength + Send + Sync,
{
    const ID: u64 = RHYTHM_GRID_ENVELOPE_ID;

    type Inputs = op!(U3 + U1 + U1 + NI);

    type Outputs = N::Outputs;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let env_value = self.step_envelope(input[0], input[1].round() as u64, input[3], input[4]);

        let inner_input = Frame::<f32, NI>::from_slice(&input[5..]);
        // Tick the inner generator and scale its output by the envelope.
        let inner_out = self.inner.tick(&inner_input);
        let mut result: Frame<f32, N::Outputs> = Frame::default();
        let env_f32 = env_value.to_f32();
        for (r, v) in result.iter_mut().zip(inner_out.iter()) {
            *r = *v * env_f32;
        }
        result
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        let mut env_values = [0.0_f32; MAX_BUFFER_SIZE];
        for (sample, env_value) in env_values.iter_mut().enumerate().take(size) {
            let env_s = self.step_envelope(
                input.at_f32(0, sample),
                input.at_f32(1, sample).round() as u64,
                input.at_f32(3, sample),
                input.at_f32(4, sample),
            );
            *env_value = env_s.to_f32();
        }

        let inner_input = input.subset(5, NI::USIZE);

        let mut inner_output = BufferArray::<N::Outputs>::new();
        self.inner
            .process(size, &inner_input, &mut inner_output.buffer_mut());

        let inner_output = inner_output.buffer_ref();
        for channel in 0..self.outputs() {
            for (sample, env_value) in env_values.iter().enumerate().take(size) {
                output.set_f32(
                    channel,
                    sample,
                    inner_output.at_f32(channel, sample) * *env_value,
                );
            }
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.pending_schedules.clear();
        self.active.clear();
    }

    fn allocate(&mut self) {
        self.inner.allocate();
    }

    fn set_hash(&mut self, hash: u64) {
        self.inner.set_hash(hash);
        self.reset();
    }
}

/// Create a rhythm grid envelope node wrapping `inner` with the given `adsr` shape.
///
/// The grid trigger is used only for beat-aligned positioning. Each enqueued schedule
/// fires exactly once; to repeat, send new start/duration inputs to enqueue the next event.
pub fn create_rhythm_grid_envelope<
    S: Real + Float + 'static,
    N: AudioNode<Inputs = NI>,
    NI: Size<S> + Size<N>,
>(
    inner: An<N>,
    adsr: AdsrShape<S>,
) -> An<RhythmGridEnvelope<S, N, NI>>
where
    UInt<UInt<UInt<UTerm, B1>, B0>, B1>: Add<NI>,
    <UInt<UInt<UInt<UTerm, B1>, B0>, B1> as Add<NI>>::Output: ArrayLength + Send + Sync,
{
    An(RhythmGridEnvelope::new(inner, adsr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    // At 100 Hz sample rate, 60 BPM → ticks_per_beat = 100.
    // Beat triggers fire at ticks 100, 200, 300, ...
    //
    // Envelope input layout (5 channels):
    //  [0] grid trigger  [1] ticks_per_beat  [2] ticks_to_next (unused)
    //  [3] start_ratio_beats  [4] duration_ratio_beats
    //
    // Schedule pulses: each entry is (tick, start_ratio_beats, duration_ratio_beats).
    // Channels 3 and 4 carry NaN on ticks where no schedule is being sent; this is the
    // sentinel for "nothing to schedule this tick".  0.0 on channel 3 means immediate
    // start (no beat offset); positive values on channel 4 set the note duration.
    fn make_input_with_tpb(
        beat_ticks: Vec<usize>,
        schedule_pulses: Vec<(usize, f32, f32)>,
        ticks_per_beat: f32,
    ) -> InputSource {
        InputSource::Generator(Box::new(move |i, ch| match ch {
            0 => {
                if beat_ticks.contains(&i) {
                    1.0
                } else {
                    0.0
                }
            }
            1 => ticks_per_beat,
            2 => 0.0,
            3 | 4 => {
                if let Some(&(_, start_ratio, duration_ratio)) =
                    schedule_pulses.iter().find(|(t, ..)| *t == i)
                {
                    if ch == 3 {
                        start_ratio
                    } else {
                        duration_ratio
                    }
                } else {
                    f32::NAN
                }
            }
            _ => 0.0,
        }))
    }

    fn make_input(beat_ticks: Vec<usize>, schedule_pulses: Vec<(usize, f32, f32)>) -> InputSource {
        make_input_with_tpb(beat_ticks, schedule_pulses, 100.0)
    }

    fn low_sr_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(100.0)
            .num_samples(num_samples)
            .with_inputs(true)
            .input_title("Grid trigger signal (1.0 on the first tick of each beat, 0.0 otherwise)")
            .input_title("Current ticks per beat as a float")
            .input_title("Current ticks to next beat as a float (unused — reserved for future use)")
            .input_title("Start beat ratio (e.g. 0.25 for quarter-beat offset)")
            .input_title("Duration beat ratio (e.g. 0.5 for half-beat, 1.5 for dotted beat)")
            .input_title("Node inputs...")
            .output_title("Node output")
            .chart_layout(Layout::CombinedPerChannelType)
            .build()
            .unwrap()
    }

    // Pulse at tick 0: tiny positive start (immediate after floor), duration=1.0 beat.
    // Beat trigger at tick 100 activates it. Envelope runs ticks 100-199 then expires.
    #[test]
    fn envelope_one_shot_immediate() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            vec![(0, 0.0, 1.0)], // start=0.0 (immediate), dur=1.0
        );
        assert_audio_unit_snapshot!(
            "envelope_one_shot_immediate",
            env,
            input,
            low_sr_config(300)
        );
    }

    // Pulse at tick 0: start=0.25 beat (25 ticks offset), duration=0.5 beat (50 ticks).
    // Beat trigger at tick 100 activates it. Envelope starts at tick 125 and runs 50 ticks.
    #[test]
    fn envelope_one_shot_with_offset() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            vec![(0, 0.25, 0.5)], // start=0.25, dur=0.5
        );
        assert_audio_unit_snapshot!(
            "envelope_one_shot_with_offset",
            env,
            input,
            low_sr_config(300)
        );
    }

    // Pulse at tick 0: event 1 (immediate, 1 beat) → activated by beat at tick 100.
    // Pulse at tick 150: event 2 (immediate, 3/4 beat) → activated by beat at tick 200.
    #[test]
    fn envelope_queue_two_events() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100, 200],
            vec![
                (0, 0.0, 1.0),    // start=0.0 (immediate), dur=1.0
                (150, 0.0, 0.75), // start=0.0 (immediate), dur=0.75
            ],
        );
        assert_audio_unit_snapshot!("envelope_queue_two_events", env, input, low_sr_config(400));
    }

    // Rounded segments with high smoothness produce eased attack/decay/release transitions.
    #[test]
    fn envelope_smoothness_rounded_curve() {
        let adsr = AdsrShape {
            attack: 0.25,
            decay: 0.25,
            sustain: 0.5,
            release: 0.25,
            smoothness: 0.9,
            decay_exponent: 1.0,
        };
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], vec![(0, 0.0, 1.0)]);
        assert_audio_unit_snapshot!(
            "envelope_smoothness_rounded_curve",
            env,
            input,
            low_sr_config(300)
        );
    }

    // Tail exponent bends both decay and release while smoothness still handles rounding.
    #[test]
    fn envelope_curved_decay_exponent() {
        let adsr = AdsrShape {
            attack: 0.2,
            decay: 0.5,
            sustain: 0.3,
            release: 0.2,
            smoothness: 0.6,
            decay_exponent: 2.0,
        };
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], vec![(0, 0.0, 1.0)]);
        assert_audio_unit_snapshot!(
            "envelope_curved_decay_exponent",
            env,
            input,
            low_sr_config(300)
        );
    }

    #[test]
    fn envelope_curved_tail_exponent_low() {
        let adsr = AdsrShape {
            attack: 0.15,
            decay: 0.45,
            sustain: 0.35,
            release: 0.35,
            smoothness: 0.6,
            decay_exponent: 0.6,
        };
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], vec![(0, 0.0, 1.0)]);
        assert_audio_unit_snapshot!(
            "envelope_curved_tail_exponent_low",
            env,
            input,
            low_sr_config(320)
        );
    }

    #[test]
    fn envelope_curved_tail_exponent_high() {
        let adsr = AdsrShape {
            attack: 0.15,
            decay: 0.45,
            sustain: 0.35,
            release: 0.35,
            smoothness: 0.6,
            decay_exponent: 2.2,
        };
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], vec![(0, 0.0, 1.0)]);
        assert_audio_unit_snapshot!(
            "envelope_curved_tail_exponent_high",
            env,
            input,
            low_sr_config(320)
        );
    }

    // A second attack while another envelope is still active is mixed, not replaced.
    #[test]
    fn envelope_overlap_attacks() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100, 200],
            vec![
                (0, 0.0, 3.0),   // 3 beats, starts at beat 100
                (150, 0.0, 1.0), // 1 beat, starts at beat 200 (overlaps event 1)
            ],
        );
        assert_audio_unit_snapshot!("envelope_overlap_attacks", env, input, low_sr_config(500));
    }

    // Duration ratios above 1.0 should produce envelopes longer than one beat.
    #[test]
    fn envelope_duration_longer_than_one_beat() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            vec![(0, 0.0, 2.5)], // 2.5 beats = 250 ticks
        );
        assert_audio_unit_snapshot!(
            "envelope_duration_longer_than_one_beat",
            env,
            input,
            low_sr_config(500)
        );
    }

    #[test]
    fn schedule_allows_decay_ratio_above_one() {
        let adsr = AdsrShape {
            attack: 0.1,
            decay: 2.5,
            sustain: 0.5,
            release: 0.1,
            smoothness: 0.0,
            decay_exponent: 1.0,
        };

        let mut env = RhythmGridEnvelope::new(dc(1.0f32), adsr);
        let _ = env.step_envelope(1.0, 100, 0.0, 1.0);

        assert_eq!(env.active.len(), 1);
        assert_eq!(env.active[0].schedule.total_duration, 100);
        assert_eq!(env.active[0].schedule.decay, 250);
    }

    // One-minute schedule at 60 BPM: 60 beat-aligned pulses over 6000 ticks.
    #[test]
    fn envelope_long_snapshot_one_minute_60bpm() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let beat_ticks: Vec<usize> = (1..=60).map(|b| b * 100).collect();
        let schedule_pulses: Vec<(usize, f32, f32)> =
            beat_ticks.iter().map(|t| (*t, 0.0, 0.25)).collect();
        let input = make_input(beat_ticks, schedule_pulses);
        assert_audio_unit_snapshot!(
            "envelope_long_snapshot_one_minute_60bpm",
            env,
            input,
            low_sr_config(6100)
        );
    }

    // Custom ADSR shape: sharp attack (20%), no decay, full sustain, long release (40%).
    #[test]
    fn envelope_custom_adsr_shape() {
        let adsr = AdsrShape {
            attack: 0.20,
            decay: 0.0,
            sustain: 1.0,
            release: 0.40,
            smoothness: 0.0,
            decay_exponent: 1.0,
        };
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], vec![(0, 0.0, 1.0)]);
        assert_audio_unit_snapshot!("envelope_custom_adsr_shape", env, input, low_sr_config(300));
    }

    // 11 events with start beat-offsets progressively increasing from 0.0 to 2.0 in steps
    // of 0.2.  Each pulse arrives 50 ticks before the activating beat trigger. Duration is
    // fixed at 0.5 beats (50 ticks).  The rightward drift of each envelope onset visualizes
    // the full 0.0 → 2.0 progression:
    //   offset 0.0  → onset immediately at beat (tick 100, 200, …)
    //   offset 1.0  → onset one full beat after the trigger
    //   offset 2.0  → onset two full beats after the trigger (tick 1300)
    // Sample rate: 100 Hz, ticks_per_beat: 100, total samples: 1500.
    #[test]
    fn envelope_progressive_start_offset_0_to_2() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let num_events: usize = 11;
        let beat_ticks: Vec<usize> = (1..=num_events).map(|b| b * 100).collect();
        let schedule_pulses: Vec<(usize, f32, f32)> = (0..num_events)
            .map(|i| {
                let tick = i * 100 + 50; // pulse 50 ticks before the beat
                let start_ratio = i as f32 * 0.2; // 0.0, 0.2, 0.4, …, 2.0
                (tick, start_ratio, 0.5) // 0.5-beat duration = 50 ticks
            })
            .collect();
        let input = make_input(beat_ticks, schedule_pulses);
        assert_audio_unit_snapshot!(
            "envelope_progressive_start_offset_0_to_2",
            env,
            input,
            low_sr_config(1500)
        );
    }

    // 11 events with both start offset and duration progressively increasing.
    // Start ratio goes from 0.0 to 2.0 (steps of 0.2), duration from 0.1 to 1.1 beats.
    // Each pulse arrives 50 ticks before its activating beat trigger.
    // The combined drift + lengthening of each envelope visualizes the full progression.
    // Sample rate: 100 Hz, ticks_per_beat: 100, total samples: 2000.
    #[test]
    fn envelope_progressive_start_and_duration_0_to_2() {
        let env = create_rhythm_grid_envelope::<f32, _, _>(dc(1.0f32), AdsrShape::default());
        let num_events: usize = 11;
        let beat_ticks: Vec<usize> = (1..=num_events).map(|b| b * 100).collect();
        let schedule_pulses: Vec<(usize, f32, f32)> = (0..num_events)
            .map(|i| {
                let tick = i * 100 + 50; // pulse 50 ticks before the beat
                let start_ratio = i as f32 * 0.2; // 0.0, 0.2, 0.4, …, 2.0
                let duration_ratio = 0.1 + i as f32 * 0.1; // 0.1, 0.2, 0.3, …, 1.1 beats
                (tick, start_ratio, duration_ratio)
            })
            .collect();
        let input = make_input(beat_ticks, schedule_pulses);
        assert_audio_unit_snapshot!(
            "envelope_progressive_start_and_duration_0_to_2",
            env,
            input,
            low_sr_config(2000)
        );
    }

    #[test]
    fn envelope_duration_range_demo_audio_sine_440() {
        const SAMPLE_RATE: f64 = 44_100.0;
        const TICKS_PER_BEAT: f32 = 22_050.0;

        let durations_beats = [0.0625_f32, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
        let beat_spacing = TICKS_PER_BEAT as usize * 10;
        let first_beat = TICKS_PER_BEAT as usize;
        let beat_ticks: Vec<usize> = (0..durations_beats.len())
            .map(|i| first_beat + i * beat_spacing)
            .collect();

        let schedule_pulses: Vec<(usize, f32, f32)> = beat_ticks
            .iter()
            .zip(durations_beats)
            .map(|(tick, duration)| (*tick, 0.0, duration))
            .collect();

        let max_duration_samples =
            (durations_beats[durations_beats.len() - 1] * TICKS_PER_BEAT) as usize;
        let total_samples = beat_ticks[beat_ticks.len() - 1] + max_duration_samples + first_beat;

        let cfg = SnapshotConfigBuilder::default()
            .sample_rate(SAMPLE_RATE)
            .num_samples(total_samples)
            .output_mode(WavOutput::Wav32)
            .build()
            .unwrap();

        let env =
            create_rhythm_grid_envelope::<f32, _, _>(sine_hz::<f32>(440.0), AdsrShape::default());
        let input = make_input_with_tpb(beat_ticks, schedule_pulses, TICKS_PER_BEAT);

        assert_audio_unit_snapshot!(
            "envelope_duration_range_demo_audio_sine_440",
            env,
            input,
            cfg
        );
    }

    #[derive(Clone)]
    struct ProcessProbeNode {
        tick_calls: Arc<AtomicUsize>,
        process_calls: Arc<AtomicUsize>,
    }

    impl AudioNode for ProcessProbeNode {
        const ID: u64 = crate::util::hash_str(concat!(module_path!(), "::ProcessProbeNode"));

        type Inputs = U0;
        type Outputs = U1;

        fn tick(&mut self, _input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
            self.tick_calls.fetch_add(1, Ordering::Relaxed);
            [0.0].into()
        }

        fn process(&mut self, size: usize, _input: &BufferRef, output: &mut BufferMut) {
            self.process_calls.fetch_add(1, Ordering::Relaxed);
            for sample in 0..size {
                output.set_f32(0, sample, 1.0);
            }
        }
    }

    #[test]
    fn envelope_process_forwards_to_inner_process() {
        type EnvelopeInputs = op!(U3 + U1 + U1);

        let tick_calls = Arc::new(AtomicUsize::new(0));
        let process_calls = Arc::new(AtomicUsize::new(0));
        let probe = ProcessProbeNode {
            tick_calls: tick_calls.clone(),
            process_calls: process_calls.clone(),
        };
        let mut env = create_rhythm_grid_envelope::<f32, _, _>(An(probe), AdsrShape::default());

        let mut input = BufferArray::<EnvelopeInputs>::new();
        for sample in 0..4 {
            input.set_f32(1, sample, 4.0);
        }
        input.set_f32(0, 0, 1.0);
        input.set_f32(3, 0, 0.0); // immediate start
        input.set_f32(4, 0, 1.0);

        let mut output = BufferArray::<U1>::new();
        env.process(4, &input.buffer_ref(), &mut output.buffer_mut());

        assert_eq!(process_calls.load(Ordering::Relaxed), 1);
        assert_eq!(tick_calls.load(Ordering::Relaxed), 0);
        assert!(output.at_f32(0, 1) > 0.0);
    }
}
