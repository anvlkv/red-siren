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

pub(crate) fn chart_snapshot_config(
    num_samples: usize,
    input_titles: &[&str],
    output_titles: &[&str],
) -> SnapshotConfig {
    let mut builder = SnapshotConfigBuilder::default();
    builder
        .num_samples(num_samples)
        .chart_layout(Layout::CombinedPerChannelType)
        .svg_width(512)
        .svg_height_per_channel(128)
        .with_inputs(true);

    for title in input_titles {
        builder.input_title(*title);
    }
    for title in output_titles {
        builder.output_title(*title);
    }

    builder.build().expect("snapshot config must be valid")
}

pub(crate) fn constant_input_by_channel(num_samples: usize, values: &[f32]) -> InputSource {
    let channels = values
        .iter()
        .map(|value| vec![*value; num_samples])
        .collect::<Vec<_>>();
    InputSource::VecByChannel(channels)
}

pub(crate) fn impulse_input(num_samples: usize) -> InputSource {
    let audio = (0..num_samples)
        .map(|i| if i == 0 { 1.0_f32 } else { 0.0 })
        .collect::<Vec<_>>();
    InputSource::VecByChannel(vec![audio])
}

pub(crate) fn impulse_with_constants_input(
    num_samples: usize,
    constant_channels: &[f32],
) -> InputSource {
    let mut channels = Vec::with_capacity(1 + constant_channels.len());
    channels.push(
        (0..num_samples)
            .map(|i| if i == 0 { 1.0_f32 } else { 0.0 })
            .collect::<Vec<_>>(),
    );
    channels.extend(
        constant_channels
            .iter()
            .map(|value| vec![*value; num_samples]),
    );
    InputSource::VecByChannel(channels)
}

pub(crate) fn linear_ramp_input(start: f32, end: f32, num_samples: usize) -> Vec<f32> {
    let steps = std::cmp::Ord::max(num_samples.saturating_sub(1), 1) as f32;
    (0..num_samples)
        .map(|i| start + (i as f32 / steps) * (end - start))
        .collect()
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
