use fundsp::prelude::*;

const NEW_YORK_COMPRESSOR_ID: u64 =
    crate::util::hash_str(concat!(module_path!(), "::NewYorkCompressor"));
const DEFAULT_ATTACK_SECS: f64 = 0.010;
const DEFAULT_RELEASE_SECS: f64 = 0.120;

#[derive(Clone)]
/// A compressor that applies compression to the input signal, while also mixing in a dry signal to retain some of the original dynamics. This is often used in parallel compression techniques, where the compressed signal is blended with the uncompressed signal to achieve a more natural sound.
///
/// Inputs:
/// - Input 0: The audio signal to be compressed.
/// - Input 1: Threshold control signal (0.0 to 1.0, where 1.0 corresponds to the maximum threshold).
/// - Input 2: Dry/Wet control signal (0.0 to 1.0, where 0.0 is fully dry and 1.0 is fully wet).
///
/// Output:
/// - Output 0: The compressed audio signal mixed with the dry signal according to the dry/wet control.
pub struct NewYorkCompressor<S: Float + Real + 'static> {
    hash: u64,
    sample_rate: S,
    attack_seconds: S,
    release_seconds: S,
    attack_coeff: S,
    release_coeff: S,
    smoothed_gain: S,
    _phantom: core::marker::PhantomData<S>,
}

impl<S: Float + Real + 'static> NewYorkCompressor<S> {
    pub fn new() -> Self {
        let sample_rate = S::from_f64(DEFAULT_SR);
        let attack_seconds = S::from_f64(DEFAULT_ATTACK_SECS);
        let release_seconds = S::from_f64(DEFAULT_RELEASE_SECS);

        let attack_coeff = Self::time_to_coeff(attack_seconds, sample_rate);
        let release_coeff = Self::time_to_coeff(release_seconds, sample_rate);

        Self {
            hash: 0,
            sample_rate,
            attack_seconds,
            release_seconds,
            attack_coeff,
            release_coeff,
            smoothed_gain: S::one(),
            _phantom: core::marker::PhantomData,
        }
    }

    #[inline]
    fn time_to_coeff(time_seconds: S, sample_rate: S) -> S {
        let min_time = S::from_f64(1.0e-6);
        let min_sr = S::from_f64(1.0);
        let tau = if time_seconds < min_time {
            min_time
        } else {
            time_seconds
        };
        let sr = if sample_rate < min_sr {
            min_sr
        } else {
            sample_rate
        };

        (-S::one() / (tau * sr)).exp()
    }

    #[inline]
    fn update_time_coefficients(&mut self) {
        self.attack_coeff = Self::time_to_coeff(self.attack_seconds, self.sample_rate);
        self.release_coeff = Self::time_to_coeff(self.release_seconds, self.sample_rate);
    }

    #[inline]
    fn soft_compress(&self, x: S, threshold: S) -> S {
        let amplitude = x.abs();

        if amplitude == S::zero() {
            return x;
        }

        let normalized_distance = (amplitude - threshold).abs() / (amplitude + threshold);
        let compressed_core = amplitude * (S::one() + normalized_distance * threshold)
            / (S::one() + normalized_distance * amplitude);

        let correction = (threshold - amplitude) / (amplitude + threshold);
        let compressed_amplitude = compressed_core
            + (threshold - compressed_core)
                * correction
                * normalized_distance
                * normalized_distance;

        if x < S::zero() {
            -compressed_amplitude
        } else {
            compressed_amplitude
        }
    }
}

impl<S: Float + Real + 'static> AudioNode for NewYorkCompressor<S> {
    const ID: u64 = NEW_YORK_COMPRESSOR_ID;

    type Inputs = U3;

    type Outputs = U1;

    fn reset(&mut self) {
        self.smoothed_gain = S::one();
    }

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let dry_signal = S::from_f32(input[0]);

        let threshold = S::from_f32(input[1].clamp(f32::EPSILON, 1.0));
        let wet_mix = S::from_f32(input[2].clamp(0.0, 1.0));

        let target_wet = self.soft_compress(dry_signal, threshold);
        let amplitude = dry_signal.abs();
        let gain_floor = S::from_f64(1.0e-12);

        let target_gain = if amplitude > gain_floor {
            target_wet.abs() / amplitude
        } else {
            S::one()
        };

        let smoothing_coeff = if target_gain < self.smoothed_gain {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        let one_minus = S::one() - smoothing_coeff;
        self.smoothed_gain = smoothing_coeff * self.smoothed_gain + one_minus * target_gain;

        let wet_signal = dry_signal * self.smoothed_gain;
        let dry_mix = S::one() - wet_mix;

        let output = dry_signal * dry_mix + wet_signal * wet_mix;
        [output.to_f32()].into()
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = S::from_f64(sample_rate);
        self.update_time_coefficients();
    }

    fn set_hash(&mut self, hash: u64) {
        self.hash = hash;
        self.reset();
    }
}

pub fn create_new_york_compressor<S: Float + Real + 'static>() -> An<NewYorkCompressor<S>> {
    An(NewYorkCompressor::new())
}

pub fn create_ny_compressor_thr_dry<S: Float + Real + 'static>(
    threshold: &Shared,
    dry_wet: &Shared,
) -> An<Pipe<Stack<Stack<Pass, Var>, Var>, NewYorkCompressor<S>>> {
    (pass() | var(threshold) | var(dry_wet)) >> create_new_york_compressor::<S>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    const SNAP_LEN: usize = 512;

    fn snapshot_config() -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(44_100.0)
            .num_samples(SNAP_LEN)
            .build()
            .unwrap()
    }

    fn stepped_audio() -> Vec<f32> {
        (0..SNAP_LEN)
            .map(|i| {
                let magnitude = match i {
                    0..64 => 0.0,
                    64..128 => 0.01,
                    128..192 => 0.05,
                    192..256 => 0.2,
                    256..320 => 0.35,
                    320..384 => 0.6,
                    384..448 => 0.85,
                    _ => 1.0,
                };

                if (i / 16) % 2 == 0 {
                    magnitude
                } else {
                    -magnitude
                }
            })
            .collect()
    }

    fn burst_audio() -> Vec<f32> {
        (0..SNAP_LEN)
            .map(|i| {
                let magnitude = match i {
                    0..160 => 0.05,
                    160..288 => 0.95,
                    _ => 0.05,
                };

                if (i / 8) % 2 == 0 {
                    magnitude
                } else {
                    -magnitude
                }
            })
            .collect()
    }

    #[test]
    fn ny_compressor_dry_passthrough() {
        let audio = stepped_audio();
        let threshold = vec![0.25; SNAP_LEN];
        let wet = vec![0.0; SNAP_LEN];

        assert_audio_unit_snapshot!(
            "ny_compressor_dry_passthrough",
            create_new_york_compressor::<f32>(),
            InputSource::VecByChannel(vec![audio, threshold, wet]),
            snapshot_config()
        );
    }

    #[test]
    fn ny_compressor_full_wet_threshold_sweep() {
        let audio = stepped_audio();
        let threshold = (0..SNAP_LEN)
            .map(|i| 0.05 + 0.9 * (i as f32 / (SNAP_LEN - 1) as f32))
            .collect::<Vec<_>>();
        let wet = vec![1.0; SNAP_LEN];

        assert_audio_unit_snapshot!(
            "ny_compressor_full_wet_threshold_sweep",
            create_new_york_compressor::<f32>(),
            InputSource::VecByChannel(vec![audio, threshold, wet]),
            snapshot_config()
        );
    }

    #[test]
    fn ny_compressor_attack_release_transient() {
        let audio = burst_audio();
        let threshold = vec![0.2; SNAP_LEN];
        let wet = vec![1.0; SNAP_LEN];

        assert_audio_unit_snapshot!(
            "ny_compressor_attack_release_transient",
            create_new_york_compressor::<f32>(),
            InputSource::VecByChannel(vec![audio, threshold, wet]),
            snapshot_config()
        );
    }
}
