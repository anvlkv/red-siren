use std::marker::PhantomData;

use fundsp::prelude::*;
use typenum::op;

const RHYTHM_GRID_ENVELOPE_ID: u64 = crate::util::hash_str("RhythmGridEnvelope");

/// ADSR shape parameters expressed as fractions of the total note duration.
///
/// `attack`, `decay`, and `release` are ratios in `[0.0, 1.0]` of the total duration.
/// `sustain` is an amplitude level in `[0.0, 1.0]`.
#[derive(Clone)]
pub struct AdsrShape {
    /// Fraction of total duration spent in attack (0 → 1). Default: 0.05
    pub attack: f32,
    /// Fraction of total duration spent in decay (1 → sustain). Default: 0.10
    pub decay: f32,
    /// Sustain amplitude level. Default: 0.7
    pub sustain: f32,
    /// Fraction of total duration spent in release (sustain → 0). Default: 0.20
    pub release: f32,
    /// Blend factor for rounded stage curves in [0.0, 1.0].
    /// 0.0 = linear ADSR segments, 1.0 = fully smoothed (smoothstep).
    pub smoothness: f32,
}

impl Default for AdsrShape {
    fn default() -> Self {
        Self {
            attack: 0.05,
            decay: 0.10,
            sustain: 0.7,
            release: 0.20,
            smoothness: 0.0,
        }
    }
}

#[derive(Clone)]
struct Schedule {
    start_ticks: u64,
    attack: u64,
    decay: u64,
    sustain: f32,
    release: u64,
    total_duration: u64,
    smoothness: f32,
}

#[derive(Clone)]
struct ActiveState {
    ticks_until_start: u64,
    envelope_tick: u64,
    schedule: Schedule,
}

#[derive(Clone)]
/// An envelope generator that produces an ADSR envelope synchronized to the rhythm grid.
///
/// ## Inputs: `U3 + U2 + U2`
///
/// ### Grid inputs: `U3`
/// - Grid trigger signal (1.0 on the first tick of each beat, 0.0 otherwise)
/// - Current ticks per beat as a float
/// - Current ticks to next beat as a float (unused — reserved for future use)
///
/// ### Scheduler start inputs: `U2` e.g. 0/1 (immediate), 1/4 (quarter-beat offset)
/// - Divisible
/// - Divisor
///
/// ### Scheduler duration inputs: `U2` e.g. 1/4 note, 3/8 note
/// - Divisible
/// - Divisor
///
/// Each tick where start/duration inputs are non-zero, a new schedule is enqueued.
/// The caller is responsible for pulsing inputs for exactly one tick per desired event.
/// The grid trigger pops one pending schedule and activates it — on the current beat if
/// the trigger and pulse arrive on the same tick, otherwise on the next beat trigger.
/// Each schedule fires exactly once; to repeat, pulse the inputs again.
///
/// ## Outputs: `N::Outputs`
pub struct RhythmGridEnvelope<S: Float, N: AudioNode> {
    inner: An<N>,
    adsr: AdsrShape,
    pending_schedules: Vec<Schedule>,
    active: Vec<ActiveState>,
    _sample_type: PhantomData<S>,
}

impl<S: Float, N: AudioNode> RhythmGridEnvelope<S, N> {
    pub fn new(inner: An<N>, adsr: AdsrShape) -> Self {
        Self {
            inner,
            adsr,
            pending_schedules: Vec::new(),
            active: Vec::new(),
            _sample_type: PhantomData,
        }
    }

    fn smooth_progress(t: f32, smoothness: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let s = smoothness.clamp(0.0, 1.0);
        let smoothstep = t * t * (3.0 - 2.0 * t);
        t + (smoothstep - t) * s
    }

    fn compute_schedule(
        adsr: &AdsrShape,
        ticks_per_beat: u64,
        start_divisible: u64,
        start_divisor: u64,
        duration_divisible: u64,
        duration_divisor: u64,
    ) -> Schedule {
        let start_ticks =
            (ticks_per_beat as f64 * start_divisible as f64 / start_divisor as f64).round() as u64;
        let total_ticks = (ticks_per_beat as f64 * duration_divisible as f64
            / duration_divisor as f64)
            .round() as u64;
        let attack = Ord::max((total_ticks as f64 * adsr.attack as f64).round() as u64, 1);
        let decay = Ord::max((total_ticks as f64 * adsr.decay as f64).round() as u64, 1);
        let release = Ord::max((total_ticks as f64 * adsr.release as f64).round() as u64, 1);
        Schedule {
            start_ticks,
            attack,
            decay,
            sustain: adsr.sustain,
            release,
            total_duration: total_ticks,
            smoothness: adsr.smoothness,
        }
    }

    fn envelope_value(tick: u64, schedule: &Schedule) -> f32 {
        if tick >= schedule.total_duration {
            return 0.0;
        }
        let attack_end = schedule.attack;
        let decay_end = attack_end + schedule.decay;
        let release_start = schedule.total_duration.saturating_sub(schedule.release);
        if tick < attack_end {
            // attack: 0 → 1
            let t = tick as f32 / schedule.attack as f32;
            Self::smooth_progress(t, schedule.smoothness)
        } else if tick < decay_end {
            // decay: 1 → sustain
            let t = (tick - attack_end) as f32 / schedule.decay as f32;
            let p = Self::smooth_progress(t, schedule.smoothness);
            1.0 - p * (1.0 - schedule.sustain)
        } else if tick < release_start {
            // sustain
            schedule.sustain
        } else {
            // release: sustain → 0
            let t = (tick - release_start) as f32 / schedule.release as f32;
            let p = Self::smooth_progress(t, schedule.smoothness);
            schedule.sustain * (1.0 - p)
        }
    }

    fn step_envelope(
        &mut self,
        grid_trigger: f32,
        ticks_per_beat: u64,
        start_divisible: u64,
        start_divisor: u64,
        duration_divisible: u64,
        duration_divisor: u64,
    ) -> f32 {
        if ticks_per_beat > 0 && start_divisor > 0 && duration_divisor > 0 && duration_divisible > 0
        {
            let sched = Self::compute_schedule(
                &self.adsr,
                ticks_per_beat,
                start_divisible,
                start_divisor,
                duration_divisible,
                duration_divisor,
            );
            self.pending_schedules.push(sched);
        }

        if grid_trigger == 1.0 && !self.pending_schedules.is_empty() {
            let sched = self.pending_schedules.remove(0);
            self.active.push(ActiveState {
                ticks_until_start: sched.start_ticks,
                envelope_tick: 0,
                schedule: sched,
            });
        }

        let mut env_value = 0.0_f32;
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

impl<S: Float, N: AudioNode> AudioNode for RhythmGridEnvelope<S, N> {
    const ID: u64 = RHYTHM_GRID_ENVELOPE_ID;

    type Inputs = op!(U3 + U2 + U2);

    type Outputs = N::Outputs;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let env_value = self.step_envelope(
            input[0],
            input[1].round() as u64,
            input[3] as u64,
            input[4] as u64,
            input[5] as u64,
            input[6] as u64,
        );

        // Tick the inner generator and scale its output by the envelope.
        let inner_out = self.inner.tick(&Frame::default());
        let mut result: Frame<f32, N::Outputs> = Frame::default();
        for (r, v) in result.iter_mut().zip(inner_out.iter()) {
            *r = *v * env_value;
        }
        result
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        let mut env_values = [0.0_f32; MAX_BUFFER_SIZE];
        for (sample, env_value) in env_values.iter_mut().enumerate().take(size) {
            *env_value = self.step_envelope(
                input.at_f32(0, sample),
                input.at_f32(1, sample).round() as u64,
                input.at_f32(3, sample) as u64,
                input.at_f32(4, sample) as u64,
                input.at_f32(5, sample) as u64,
                input.at_f32(6, sample) as u64,
            );
        }

        let mut inner_output = BufferArray::<N::Outputs>::new();
        self.inner
            .process(size, &BufferRef::empty(), &mut inner_output.buffer_mut());

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
}

/// Create a rhythm grid envelope node wrapping `inner` with the given `adsr` shape.
///
/// The grid trigger is used only for beat-aligned positioning. Each enqueued schedule
/// fires exactly once; to repeat, send new start/duration inputs to enqueue the next event.
pub fn rhythm_grid_envelope<S: Float, N: AudioNode>(
    inner: An<N>,
    adsr: AdsrShape,
) -> An<RhythmGridEnvelope<S, N>> {
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
    // Envelope input layout (7 channels):
    //  [0] grid trigger  [1] ticks_per_beat  [2] ticks_to_next (unused)
    //  [3] start_div  [4] start_sor  [5] dur_div  [6] dur_sor
    //
    // Schedule pulses: each entry is (tick, start_div, start_sor, dur_div, dur_sor).
    // Inputs are non-zero only on the exact pulse tick; zero on all other ticks.
    fn make_input(
        beat_ticks: Vec<usize>,
        schedule_pulses: Vec<(usize, f32, f32, f32, f32)>,
    ) -> InputSource {
        InputSource::Generator(Box::new(move |i, ch| match ch {
            0 => {
                if beat_ticks.contains(&i) {
                    1.0
                } else {
                    0.0
                }
            }
            1 => 100.0,
            2 => 0.0,
            _ => {
                if let Some(&(_, sd, ss, dd, ds)) = schedule_pulses.iter().find(|(t, ..)| *t == i) {
                    match ch {
                        3 => sd,
                        4 => ss,
                        5 => dd,
                        6 => ds,
                        _ => 0.0,
                    }
                } else {
                    0.0
                }
            }
        }))
    }

    fn low_sr_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(100.0)
            .num_samples(num_samples)
            .build()
            .unwrap()
    }

    // Pulse at tick 0: start=0/1 (immediate), duration=1/1 (one beat = 100 ticks).
    // Beat trigger at tick 100 activates it. Envelope runs ticks 100-199 then expires.
    #[test]
    fn envelope_one_shot_immediate() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            vec![(0, 0.0, 1.0, 1.0, 1.0)], // start=0/1, dur=1/1
        );
        assert_audio_unit_snapshot!(
            "envelope_one_shot_immediate",
            env,
            input,
            low_sr_config(300)
        );
    }

    // Pulse at tick 0: start=1/4 beat (25 ticks offset), duration=1/2 beat (50 ticks).
    // Beat trigger at tick 100 activates it. Envelope starts at tick 125 and runs 50 ticks.
    #[test]
    fn envelope_one_shot_with_offset() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            vec![(0, 1.0, 4.0, 1.0, 2.0)], // start=1/4, dur=1/2
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
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100, 200],
            vec![
                (0, 0.0, 1.0, 1.0, 1.0),   // start=0/1, dur=1/1
                (150, 0.0, 1.0, 3.0, 4.0), // start=0/1, dur=3/4
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
        };
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], vec![(0, 0.0, 1.0, 1.0, 1.0)]);
        assert_audio_unit_snapshot!(
            "envelope_smoothness_rounded_curve",
            env,
            input,
            low_sr_config(300)
        );
    }

    // A second attack while another envelope is still active is mixed, not replaced.
    #[test]
    fn envelope_overlap_attacks() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100, 200],
            vec![
                (0, 0.0, 1.0, 3.0, 1.0),   // 3 beats, starts at beat 100
                (150, 0.0, 1.0, 1.0, 1.0), // 1 beat, starts at beat 200 (overlaps event 1)
            ],
        );
        assert_audio_unit_snapshot!("envelope_overlap_attacks", env, input, low_sr_config(500));
    }

    // Duration ratios above 1.0 should produce envelopes longer than one beat.
    #[test]
    fn envelope_duration_longer_than_one_beat() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            vec![(0, 0.0, 1.0, 5.0, 2.0)], // 5/2 beats = 250 ticks
        );
        assert_audio_unit_snapshot!(
            "envelope_duration_longer_than_one_beat",
            env,
            input,
            low_sr_config(500)
        );
    }

    // One-minute schedule at 60 BPM: 60 beat-aligned pulses over 6000 ticks.
    #[test]
    fn envelope_long_snapshot_one_minute_60bpm() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let beat_ticks: Vec<usize> = (1..=60).map(|b| b * 100).collect();
        let schedule_pulses: Vec<(usize, f32, f32, f32, f32)> = beat_ticks
            .iter()
            .map(|t| (*t, 0.0, 1.0, 1.0, 4.0))
            .collect();
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
        };
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], vec![(0, 0.0, 1.0, 1.0, 1.0)]);
        assert_audio_unit_snapshot!("envelope_custom_adsr_shape", env, input, low_sr_config(300));
    }

    #[derive(Clone)]
    struct ProcessProbeNode {
        tick_calls: Arc<AtomicUsize>,
        process_calls: Arc<AtomicUsize>,
    }

    impl AudioNode for ProcessProbeNode {
        const ID: u64 = crate::util::hash_str("ProcessProbeNode");

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
        type EnvelopeInputs = op!(U3 + U2 + U2);

        let tick_calls = Arc::new(AtomicUsize::new(0));
        let process_calls = Arc::new(AtomicUsize::new(0));
        let probe = ProcessProbeNode {
            tick_calls: tick_calls.clone(),
            process_calls: process_calls.clone(),
        };
        let mut env = rhythm_grid_envelope::<f32, _>(An(probe), AdsrShape::default());

        let mut input = BufferArray::<EnvelopeInputs>::new();
        for sample in 0..4 {
            input.set_f32(1, sample, 4.0);
        }
        input.set_f32(0, 0, 1.0);
        input.set_f32(4, 0, 1.0);
        input.set_f32(5, 0, 1.0);
        input.set_f32(6, 0, 1.0);

        let mut output = BufferArray::<U1>::new();
        env.process(4, &input.buffer_ref(), &mut output.buffer_mut());

        assert_eq!(process_calls.load(Ordering::Relaxed), 1);
        assert_eq!(tick_calls.load(Ordering::Relaxed), 0);
        assert!(output.at_f32(0, 1) > 0.0);
    }
}
