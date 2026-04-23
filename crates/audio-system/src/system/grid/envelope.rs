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
}

impl Default for AdsrShape {
    fn default() -> Self {
        Self {
            attack: 0.05,
            decay: 0.10,
            sustain: 0.7,
            release: 0.20,
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
/// Each time the start/duration inputs change to a new non-zero value, a new schedule
/// is appended to the pending queue. The grid trigger uses the next beat position to pop
/// one schedule from the queue and activate it. Each schedule fires exactly once.
/// To repeat, send new inputs to enqueue the next occurrence.
///
/// ## Outputs: `N::Outputs`
pub struct RhythmGridEnvelope<S: Float, N: AudioNode> {
    inner: An<N>,
    adsr: AdsrShape,
    pending_schedules: Vec<Schedule>,
    active: Option<ActiveState>,
    last_inputs: Option<[u64; 4]>,
    _sample_type: PhantomData<S>,
}

impl<S: Float, N: AudioNode> RhythmGridEnvelope<S, N> {
    pub fn new(inner: An<N>, adsr: AdsrShape) -> Self {
        Self {
            inner,
            adsr,
            pending_schedules: Vec::new(),
            active: None,
            last_inputs: None,
            _sample_type: PhantomData,
        }
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
            tick as f32 / schedule.attack as f32
        } else if tick < decay_end {
            // decay: 1 → sustain
            let t = (tick - attack_end) as f32 / schedule.decay as f32;
            1.0 - t * (1.0 - schedule.sustain)
        } else if tick < release_start {
            // sustain
            schedule.sustain
        } else {
            // release: sustain → 0
            let t = (tick - release_start) as f32 / schedule.release as f32;
            schedule.sustain * (1.0 - t)
        }
    }
}

impl<S: Float, N: AudioNode> AudioNode for RhythmGridEnvelope<S, N> {
    const ID: u64 = RHYTHM_GRID_ENVELOPE_ID;

    type Inputs = op!(U3 + U2 + U2);

    type Outputs = N::Outputs;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let grid_trigger = input[0];
        let ticks_per_beat = input[1].round() as u64;
        let start_divisible = input[3] as u64;
        let start_divisor = input[4] as u64;
        let duration_divisible = input[5] as u64;
        let duration_divisor = input[6] as u64;

        // When the start/duration inputs change to a valid new value, enqueue a new schedule.
        if ticks_per_beat > 0 && start_divisor > 0 && duration_divisor > 0 {
            let new_inputs = [
                start_divisible,
                start_divisor,
                duration_divisible,
                duration_divisor,
            ];
            if self.last_inputs != Some(new_inputs) {
                self.last_inputs = Some(new_inputs);
                let sched = Self::compute_schedule(
                    &self.adsr.clone(),
                    ticks_per_beat,
                    start_divisible,
                    start_divisor,
                    duration_divisible,
                    duration_divisor,
                );
                self.pending_schedules.push(sched);
            }
        }

        // On beat trigger: pop the next pending schedule and activate it. Each fires once.
        if grid_trigger == 1.0 && !self.pending_schedules.is_empty() {
            let sched = self.pending_schedules.remove(0);
            self.active = Some(ActiveState {
                ticks_until_start: sched.start_ticks,
                envelope_tick: 0,
                schedule: sched,
            });
        }

        // Advance the active envelope and read its current value.
        let env_value = if let Some(active) = &mut self.active {
            if active.ticks_until_start > 0 {
                active.ticks_until_start -= 1;
                0.0_f32
            } else {
                let val = Self::envelope_value(active.envelope_tick, &active.schedule);
                active.envelope_tick += 1;
                val
            }
        } else {
            0.0_f32
        };

        // Expire a finished envelope.
        if matches!(&self.active, Some(a) if a.envelope_tick >= a.schedule.total_duration) {
            self.active = None;
        }

        // Tick the inner generator and scale its output by the envelope.
        let inner_out = self.inner.tick(&Frame::default());
        let mut result: Frame<f32, N::Outputs> = Frame::default();
        for (r, v) in result.iter_mut().zip(inner_out.iter()) {
            *r = *v * env_value;
        }
        result
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.pending_schedules.clear();
        self.active = None;
        self.last_inputs = None;
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

    // At 100 Hz sample rate, 60 BPM → ticks_per_beat = 100.
    // First beat trigger fires at tick 100.
    //
    // Envelope input layout (7 channels):
    //  [0] grid trigger  [1] ticks_per_beat  [2] ticks_to_next (unused)
    //  [3] start_div  [4] start_sor  [5] dur_div  [6] dur_sor
    fn make_input(
        trigger_ticks: Vec<usize>,
        start_div_fn: impl Fn(usize) -> f32 + 'static,
        start_sor_fn: impl Fn(usize) -> f32 + 'static,
        dur_div_fn: impl Fn(usize) -> f32 + 'static,
        dur_sor_fn: impl Fn(usize) -> f32 + 'static,
    ) -> InputSource {
        InputSource::Generator(Box::new(move |i, ch| match ch {
            0 => {
                if trigger_ticks.contains(&i) {
                    1.0
                } else {
                    0.0
                }
            }
            1 => 100.0,
            2 => 0.0,
            3 => start_div_fn(i),
            4 => start_sor_fn(i),
            5 => dur_div_fn(i),
            6 => dur_sor_fn(i),
            _ => 0.0,
        }))
    }

    fn low_sr_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(100.0)
            .num_samples(num_samples)
            .build()
            .unwrap()
    }

    // Enqueue one event: start=0/1 (immediate), duration=1/1 (one beat = 100 ticks).
    // Trigger at tick 100. Envelope runs ticks 100-199 then expires.
    #[test]
    fn envelope_one_shot_immediate() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            |_| 0.0, // start_div = 0 (immediate)
            |_| 1.0, // start_sor = 1
            |_| 1.0, // dur_div = 1
            |_| 1.0, // dur_sor = 1 → 1 beat
        );
        assert_audio_unit_snapshot!(
            "envelope_one_shot_immediate",
            env,
            input,
            low_sr_config(300)
        );
    }

    // Enqueue one event: start=1/4 beat (25 ticks offset), duration=1/2 beat (50 ticks).
    // Trigger at tick 100. Envelope starts at tick 125 and runs 50 ticks.
    #[test]
    fn envelope_one_shot_with_offset() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100],
            |_| 1.0, // start_div = 1
            |_| 4.0, // start_sor = 4 → 1/4 beat = 25 ticks offset
            |_| 1.0, // dur_div = 1
            |_| 2.0, // dur_sor = 2 → 1/2 beat = 50 ticks
        );
        assert_audio_unit_snapshot!(
            "envelope_one_shot_with_offset",
            env,
            input,
            low_sr_config(300)
        );
    }

    // Enqueue two events by changing inputs at tick 50.
    // Trigger at tick 100 fires the first (immediate, 1 beat).
    // Trigger at tick 200 fires the second (1/4 offset, 3/4 beat).
    #[test]
    fn envelope_queue_two_events() {
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), AdsrShape::default());
        let input = make_input(
            vec![100, 200],
            |i| if i < 50 { 0.0 } else { 1.0 }, // start_div: 0 then 1
            |i| if i < 50 { 1.0 } else { 4.0 }, // start_sor: 1 then 4
            |_| 1.0,                            // dur_div = 1
            |i| if i < 50 { 1.0 } else { 2.0 }, // dur_sor: 1 beat then 1/2 beat
        );
        assert_audio_unit_snapshot!("envelope_queue_two_events", env, input, low_sr_config(400));
    }

    // Custom ADSR shape: sharp attack (20%), no decay, full sustain, long release (40%).
    #[test]
    fn envelope_custom_adsr_shape() {
        let adsr = AdsrShape {
            attack: 0.20,
            decay: 0.0,
            sustain: 1.0,
            release: 0.40,
        };
        let env = rhythm_grid_envelope::<f32, _>(dc(1.0f32), adsr);
        let input = make_input(vec![100], |_| 0.0, |_| 1.0, |_| 1.0, |_| 1.0);
        assert_audio_unit_snapshot!("envelope_custom_adsr_shape", env, input, low_sr_config(300));
    }
}
