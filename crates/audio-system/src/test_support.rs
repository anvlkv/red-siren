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

pub(crate) fn processing_snapshot_config(
    processing_mode: Processing,
    warm_up: WarmUp,
    num_samples: usize,
    allow_abnormal_samples: bool,
) -> SnapshotConfig {
    SnapshotConfigBuilder::default()
        .sample_rate(44_100.0)
        .num_samples(num_samples)
        .with_inputs(true)
        .warm_up(warm_up)
        .processing_mode(processing_mode)
        .allow_abnormal_samples(allow_abnormal_samples)
        .show_grid(true)
        .svg_width(1024)
        .svg_height_per_channel(512)
        .chart_layout(Layout::CombinedPerChannelType)
        .build()
        .expect("snapshot config must be valid")
}

/// Stereo input: left = sine, right = cosine (L != R so M and S are non-trivial).
pub(crate) fn stereo_sine_cosine_input(num_samples: usize) -> InputSource {
    let left: Vec<f32> = (0..num_samples)
        .map(|i| (2.0 * std::f32::consts::PI * i as f32 / num_samples as f32).sin())
        .collect();
    let right: Vec<f32> = (0..num_samples)
        .map(|i| (2.0 * std::f32::consts::PI * i as f32 / num_samples as f32).cos())
        .collect();
    InputSource::VecByChannel(vec![left, right])
}

/// Stacks `channels` sine oscillators at base_freq, 2*base_freq, 3*base_freq ...
/// into a multi-output AudioUnit initialized for InputSource::AudioUnit.
pub(crate) fn initialized_sine_driver(
    channels: usize,
    sample_rate: f64,
    base_freq_hz: f32,
) -> Box<dyn AudioUnit> {
    let mut net = Net::new(0, channels);
    for ch in 0..channels {
        let freq = base_freq_hz * (ch as f32 + 1.0);
        let node = net.push(Box::new(sine_hz::<f32>(freq)));
        net.connect_output(node, 0, ch);
    }

    let mut unit: Box<dyn AudioUnit> = Box::new(net);
    unit.set_sample_rate(sample_rate);
    unit.reset();
    unit
}
