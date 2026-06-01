#![cfg(test)]

use fundsp::prelude::*;
use insta_fun::prelude::*;

pub(crate) fn low_sr_snapshot_config(num_samples: usize) -> SnapshotConfig {
    SnapshotConfigBuilder::default()
        .sample_rate(100.0)
        .num_samples(num_samples)
        .build()
        .expect("snapshot config must be valid")
}

/// Stereo input: left = sine, right = cosine (L != R so M and S are non-trivial).
pub(crate) fn stereo_sine_cosine_input() -> InputSource {
    InputSource::AudioUnit(Box::new(sine_hz::<f32>(440.0) >> split::<U2>()))
}
