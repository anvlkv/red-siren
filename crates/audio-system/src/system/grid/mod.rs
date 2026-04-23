use std::marker::PhantomData;

use fundsp::prelude::*;

mod envelope;
pub use envelope::{rhythm_grid_envelope, AdsrShape, RhythmGridEnvelope};

const RHYTHM_GRID_ID: u64 = crate::util::hash_str("RhythmGrid");

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
pub struct RhythmGrid<S: Float> {
    sample_rate: f64,
    current_bpm: u64,
    current_ticks_per_beat: u64,
    ticks_to_beat_remaining: u64,
    _sample_type: PhantomData<S>,
}

impl<S: Float> RhythmGrid<S> {
    pub fn new() -> Self {
        Self {
            sample_rate: DEFAULT_SR,
            current_bpm: 0,
            current_ticks_per_beat: 0,
            ticks_to_beat_remaining: 0,
            _sample_type: PhantomData,
        }
    }

    fn compute_ticks_per_beat(&self, target_bpm: u64) -> u64 {
        ((convert::<f64, S>(self.sample_rate) * convert::<f64, S>(60_f64))
            / convert(target_bpm as f32))
        .round()
        .to_i64()
        .unsigned_abs()
    }

    fn apply_bpm_change(&mut self, new_bpm: u64) {
        let new_tpb = self.compute_ticks_per_beat(new_bpm);
        if self.current_ticks_per_beat > 0 {
            self.ticks_to_beat_remaining = (self.ticks_to_beat_remaining as f64 * new_tpb as f64
                / self.current_ticks_per_beat as f64)
                .round() as u64;
        } else {
            self.ticks_to_beat_remaining = new_tpb;
        }
        self.current_bpm = new_bpm;
        self.current_ticks_per_beat = new_tpb;
    }
}

impl<S: Float> AudioNode for RhythmGrid<S> {
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
                self.ticks_to_beat_remaining =
                    (self.ticks_to_beat_remaining as f64 * new_tpb as f64 / old_tpb as f64).round()
                        as u64;
            } else {
                self.ticks_to_beat_remaining = new_tpb;
            }
            self.current_ticks_per_beat = new_tpb;
        }
    }
}

/// Create a rhythm grid node. Outputs 1.0 on the first tick of each beat, 0.0 otherwise.
/// Input is BPM as a float.
pub fn rhythm_grid<S: Float>() -> An<RhythmGrid<S>> {
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
            InputSource::Generator(Box::new(|i, _| {
                if i < 150 {
                    60.0
                } else {
                    120.0
                }
            })),
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
