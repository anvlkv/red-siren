use std::marker::PhantomData;

use fundsp::prelude::*;
use num_rational::Ratio;

mod envelope;
pub use envelope::{AdsrShape, RhythmGridEnvelope, create_rhythm_grid_envelope};

const RHYTHM_GRID_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::RhythmGrid"));

pub const RATIO_SCALE: i64 = 1_000_000;

#[derive(Clone)]
/// A rhythm grid that outputs a trigger signal (1.0) on the first tick of each beat, and 0.0 otherwise.
/// The BPM can be dynamically changed by changing the input value. The grid will adjust its internal timing accordingly, ensuring that the trigger signals remain in sync with the new BPM.
///
/// Inputs: 1
///  - BPM constant as a float
///
/// Outputs: 3
/// - Trigger signal: 1.0 on the first tick of each beat, 0.0 otherwise
/// - Current ticks per beat as a float
/// - Current ticks to next beat as a float
pub struct RhythmGrid<S: Real + Float> {
    sample_rate: f64,
    current_bpm: u64,
    current_ticks_per_beat: u64,
    ticks_to_beat_remaining: u64,
    _sample_type: PhantomData<S>,
}

impl<S: Real + Float> RhythmGrid<S> {
    pub fn new() -> Self {
        Self {
            sample_rate: DEFAULT_SR,
            current_bpm: 0,
            current_ticks_per_beat: 0,
            ticks_to_beat_remaining: 0,
            _sample_type: PhantomData,
        }
    }

    fn as_i64(value: u64) -> i64 {
        i64::try_from(value).unwrap_or(i64::MAX)
    }

    fn ratio_from_positive_f64(value: f64) -> Ratio<i64> {
        if !value.is_finite() || value <= 0.0 {
            return Ratio::from_integer(0);
        }
        let scaled = (value * RATIO_SCALE as f64).round() as i64;
        Ratio::new(scaled, RATIO_SCALE)
    }

    fn ratio_floor_u64(value: &Ratio<i64>) -> u64 {
        let floored = value.floor().to_integer();
        if floored <= 0 { 0 } else { floored as u64 }
    }

    fn ratio_ceil_u64(value: &Ratio<i64>) -> u64 {
        let ceiled = value.ceil().to_integer();
        if ceiled <= 0 { 0 } else { ceiled as u64 }
    }

    fn rescale_remaining_ticks_floor(remaining: u64, new_total: u64, old_total: u64) -> u64 {
        if old_total == 0 {
            return new_total;
        }
        let ratio = Ratio::from_integer(Self::as_i64(remaining))
            * Ratio::from_integer(Self::as_i64(new_total))
            / Ratio::from_integer(Self::as_i64(old_total));
        Self::ratio_floor_u64(&ratio)
    }

    fn compute_ticks_per_beat(&self, target_bpm: u64) -> u64 {
        if target_bpm == 0 {
            return 0;
        }
        let sample_rate_ratio = Self::ratio_from_positive_f64(self.sample_rate);
        let ticks_ratio = sample_rate_ratio * Ratio::new(60, Self::as_i64(target_bpm));
        Ord::max(Self::ratio_ceil_u64(&ticks_ratio), 1)
    }

    fn apply_bpm_change(&mut self, new_bpm: u64) {
        let new_tpb = self.compute_ticks_per_beat(new_bpm);
        if self.current_ticks_per_beat > 0 {
            self.ticks_to_beat_remaining = Self::rescale_remaining_ticks_floor(
                self.ticks_to_beat_remaining,
                new_tpb,
                self.current_ticks_per_beat,
            );
        } else {
            self.ticks_to_beat_remaining = new_tpb;
        }
        self.current_bpm = new_bpm;
        self.current_ticks_per_beat = new_tpb;
    }
}

impl<S: Real + Float> AudioNode for RhythmGrid<S> {
    const ID: u64 = RHYTHM_GRID_ID;

    type Inputs = U1;

    type Outputs = U3;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let target_bpm = input[0].round() as u64;

        if target_bpm != self.current_bpm {
            if target_bpm > 0 {
                self.apply_bpm_change(target_bpm);
            } else {
                self.current_bpm = 0;
                self.current_ticks_per_beat = 0;
                self.ticks_to_beat_remaining = 0;
            }
        }

        if self.current_ticks_per_beat == 0 {
            return [0.0, 0.0, 0.0].into();
        }

        if self.ticks_to_beat_remaining == 0 {
            self.ticks_to_beat_remaining = self.current_ticks_per_beat - 1;
            [
                1.0,
                self.current_ticks_per_beat as f32,
                self.ticks_to_beat_remaining as f32,
            ]
            .into()
        } else {
            self.ticks_to_beat_remaining -= 1;
            [
                0.0,
                self.current_ticks_per_beat as f32,
                self.ticks_to_beat_remaining as f32,
            ]
            .into()
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let old_tpb = self.current_ticks_per_beat;
        self.sample_rate = sample_rate;
        if self.current_bpm > 0 {
            let new_tpb = self.compute_ticks_per_beat(self.current_bpm);
            if old_tpb > 0 {
                self.ticks_to_beat_remaining = Self::rescale_remaining_ticks_floor(
                    self.ticks_to_beat_remaining,
                    new_tpb,
                    old_tpb,
                );
            } else {
                self.ticks_to_beat_remaining = new_tpb;
            }
            self.current_ticks_per_beat = new_tpb;
        }
    }
}

/// Create a rhythm grid node. Outputs 1.0 on the first tick of each beat, 0.0 otherwise.
/// Input is BPM as a float.
pub fn rhythm_grid<S: Real + Float>() -> An<RhythmGrid<S>> {
    An(RhythmGrid::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    fn low_sr_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(100.0)
            .num_samples(num_samples)
            .build()
            .unwrap()
    }

    #[test]
    fn ratio_rounding_clamps_non_positive_values() {
        let positive = Ratio::new(7_i64, 3_i64);
        let zero = Ratio::new(0_i64, 1_i64);
        let negative = Ratio::new(-7_i64, 3_i64);

        assert_eq!(RhythmGrid::<f32>::ratio_floor_u64(&positive), 2);
        assert_eq!(RhythmGrid::<f32>::ratio_ceil_u64(&positive), 3);
        assert_eq!(RhythmGrid::<f32>::ratio_floor_u64(&zero), 0);
        assert_eq!(RhythmGrid::<f32>::ratio_ceil_u64(&zero), 0);
        assert_eq!(RhythmGrid::<f32>::ratio_floor_u64(&negative), 0);
        assert_eq!(RhythmGrid::<f32>::ratio_ceil_u64(&negative), 0);
    }

    // At 100 Hz sample rate, 60 BPM → ticks_per_beat = 100.
    // The first trigger fires at tick 100 (not 0), then every 100 ticks after.
    #[test]
    fn grid_steady_60bpm() {
        let grid = rhythm_grid::<f32>();
        assert_audio_unit_snapshot!(
            "grid_steady_60bpm",
            grid,
            InputSource::Flat(vec![60.0]),
            low_sr_config(400)
        );
    }

    // Start at 60 BPM (tpb=100), switch to 120 BPM (tpb=50) at tick 150.
    // The remaining time in the current beat is rescaled on the BPM change.
    #[test]
    fn grid_bpm_change() {
        let grid = rhythm_grid::<f32>();
        assert_audio_unit_snapshot!(
            "grid_bpm_change",
            grid,
            InputSource::Generator(Box::new(|i, _| { if i < 150 { 60.0 } else { 120.0 } })),
            low_sr_config(400)
        );
    }

    // Zero BPM input → all outputs are 0.
    #[test]
    fn grid_zero_bpm() {
        let grid = rhythm_grid::<f32>();
        assert_audio_unit_snapshot!(
            "grid_zero_bpm",
            grid,
            InputSource::Flat(vec![0.0]),
            low_sr_config(200)
        );
    }
}
