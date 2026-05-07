#![cfg(test)]

use std::cell::RefCell;
use std::rc::Rc;

use common::instrument::{
    config::{config_test_cases, representative_layout_configs},
    Config as InstrumentConfig,
};
use common::tuner::{Config as TunerConfig, SensorData};
use fundsp::prelude::*;
use insta_fun::prelude::*;

use crate::rt::ExcitementSource;
use crate::system::{create_system, excitor::FFT_WINDOW_SIZE, values::FineTunedValues};

fn tuner_config_for(instrument_config: &InstrumentConfig) -> TunerConfig {
    let sensor_data = instrument_config
        .0
        .iter()
        .flat_map(|band| band.nodes.iter())
        .map(|node| {
            let center = node.frequency as f32;
            SensorData {
                key: node.key,
                min_frequency: (center * 0.95).max(1.0),
                max_frequency: (center * 1.05).max(1.0 + f32::EPSILON),
                min_magnitude: 0.0,
                max_magnitude: 1.0,
            }
        })
        .collect::<Vec<_>>();

    TunerConfig {
        sensor_data,
        sample_rate: 44_100.0,
        fft_size: FFT_WINDOW_SIZE,
        ..TunerConfig::default()
    }
}

fn two_sine_mic_input(sample_rate: f64) -> InputSource {
    // (sine_hz(110) + sine_hz(220)) * 0.5  — single mono output channel
    let graph = (sine_hz::<f32>(110.0) + sine_hz::<f32>(220.0)) * constant(0.5_f32);
    let mut unit: Box<dyn AudioUnit> = Box::new(graph);
    unit.set_sample_rate(sample_rate);
    unit.reset();
    InputSource::AudioUnit(unit)
}

fn system_snapshot_config(num_samples: usize, warmup_input: InputSource) -> SnapshotConfig {
    SnapshotConfigBuilder::default()
        .sample_rate(44_100.0)
        .num_samples(num_samples)
        .output_assertion(OutputAssertion::Skip)
        .allow_abnormal_samples(true)
        .warm_up(WarmUp::SamplesWithInput {
            samples: FFT_WINDOW_SIZE,
            input: Rc::new(RefCell::new(warmup_input)),
        })
        .processing_mode(Processing::Tick)
        .with_inputs(true)
        .show_grid(true)
        .svg_width(1024)
        .svg_height_per_channel(256)
        .chart_layout(Layout::CombinedPerChannelType)
        .build()
        .expect("snapshot config must be valid")
}

fn system_batch_snapshot_config(num_samples: usize, warmup_input: InputSource) -> SnapshotConfig {
    SnapshotConfigBuilder::default()
        .sample_rate(44_100.0)
        .num_samples(num_samples)
        .output_assertion(OutputAssertion::Skip)
        .allow_abnormal_samples(true)
        .warm_up(WarmUp::SamplesWithInput {
            samples: FFT_WINDOW_SIZE,
            input: Rc::new(RefCell::new(warmup_input)),
        })
        .processing_mode(Processing::Batch(64))
        .with_inputs(true)
        .show_grid(true)
        .svg_width(1024)
        .svg_height_per_channel(256)
        .chart_layout(Layout::CombinedPerChannelType)
        .build()
        .expect("snapshot config must be valid")
}

fn assert_system_meta(label: &str, instrument_config: &InstrumentConfig, batch: bool) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime must be created");
    let _guard = runtime.enter();

    let tuner_config = tuner_config_for(instrument_config);
    let values = FineTunedValues::new();

    let (net, handle) = create_system::<f32>(
        2,
        ExcitementSource::Mic,
        instrument_config,
        &tuner_config,
        &values,
        Some(42),
    );

    // Open the input NY path for mic-driven excitation during snapshot runs.
    handle.input_ny_thr.set_value(0.0);
    handle.input_ny_wd.set_value(1.0);

    let expected_outputs = net.outputs();
    let expected_inputs = net.inputs();
    let snapshot_samples = FFT_WINDOW_SIZE * 4;

    let config = if batch {
        system_batch_snapshot_config(
            snapshot_samples,
            two_sine_mic_input(tuner_config.sample_rate as f64),
        )
    } else {
        system_snapshot_config(
            snapshot_samples,
            two_sine_mic_input(tuner_config.sample_rate as f64),
        )
    };

    let mode_label = if batch { "batch" } else { "tick" };
    let suffix = format!("{label}_{mode_label}");

    insta::with_settings!({ snapshot_suffix => suffix }, {
        assert_audio_unit_meta_data_snapshot!(
            net,
            two_sine_mic_input(tuner_config.sample_rate as f64),
            config => |data: &AudioUnitSnapshotData| {
                assert_eq!(data.output_data.len(), expected_outputs, "output channel count must match");
                assert_eq!(data.num_samples, snapshot_samples, "num samples must match");

                let min_bucket = |v: f32| if v < 0.0 { -1.0_f64 } else { 0.0_f64 };
                let max_bucket = |v: f32| if v > 0.0 { 1.0_f64 } else { 0.0_f64 };

                let mut channel_mins = Vec::with_capacity(data.output_data.len());
                let mut channel_maxes = Vec::with_capacity(data.output_data.len());

                for samples in &data.output_data {
                    let (min, max) = samples.iter().filter(|v| v.is_finite()).fold(
                        (f32::INFINITY, f32::NEG_INFINITY),
                        |(mn, mx), &v| (mn.min(v), mx.max(v)),
                    );
                    if min.is_finite() && max.is_finite() {
                        channel_mins.push(min_bucket(min));
                        channel_maxes.push(max_bucket(max));
                    } else {
                        channel_mins.push(0.0);
                        channel_maxes.push(0.0);
                    }
                }

                let abnormal_count: usize = data.abnormalities.iter().map(|ch| ch.len()).sum();

                insta_fun_meta! {
                    output_channels: scalar(expected_outputs),
                    input_channels: scalar(expected_inputs),
                    abnormal_samples: scalar(abnormal_count),
                    output_min_per_channel: line(channel_mins),
                    output_max_per_channel: line(channel_maxes),
                }
            }
        );
    });
}

#[test]
fn system_mic_tick_first_layout() {
    let configs = representative_layout_configs();
    assert_system_meta("layout_first", &configs[0], false);
}

#[test]
fn system_mic_tick_mid_layout() {
    let configs = representative_layout_configs();
    let mid = configs.len() / 2;
    assert_system_meta("layout_mid", &configs[mid], false);
}

#[test]
fn system_mic_tick_last_layout() {
    let configs = representative_layout_configs();
    let last = configs.len() - 1;
    assert_system_meta("layout_last", &configs[last], false);
}

#[test]
fn system_mic_batch_first_layout() {
    let configs = representative_layout_configs();
    assert_system_meta("layout_first", &configs[0], true);
}

#[test]
fn system_mic_batch_mid_layout() {
    let configs = representative_layout_configs();
    let mid = configs.len() / 2;
    assert_system_meta("layout_mid", &configs[mid], true);
}

#[test]
fn system_mic_batch_last_layout() {
    let configs = representative_layout_configs();
    let last = configs.len() - 1;
    assert_system_meta("layout_last", &configs[last], true);
}

#[test]
fn system_builds_for_all_layout_test_cases() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime must be created");
    let _guard = runtime.enter();

    let values = FineTunedValues::new();

    for (instrument_config, _layout) in config_test_cases() {
        let tuner_config = tuner_config_for(&instrument_config);

        let (_net, _handle) = create_system::<f32>(
            2,
            ExcitementSource::Mic,
            &instrument_config,
            &tuner_config,
            &values,
            Some(42),
        );
    }
}

#[test]
fn system_builds_for_supported_output_channel_counts() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime must be created");
    let _guard = runtime.enter();

    let instrument_config = representative_layout_configs()
        .into_iter()
        .last()
        .expect("representative layout config");
    let tuner_config = tuner_config_for(&instrument_config);
    let values = FineTunedValues::new();

    for num_channels in 1..=8 {
        let (_net, _handle) = create_system::<f32>(
            num_channels,
            ExcitementSource::Mic,
            &instrument_config,
            &tuner_config,
            &values,
            Some(42),
        );
    }
}

#[test]
fn system_builds_for_entropy_source() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime must be created");
    let _guard = runtime.enter();

    let instrument_config = representative_layout_configs()
        .into_iter()
        .last()
        .expect("representative layout config");
    let tuner_config = tuner_config_for(&instrument_config);
    let values = FineTunedValues::new();

    let (_net, _handle) = create_system::<f32>(
        2,
        ExcitementSource::Entropy,
        &instrument_config,
        &tuner_config,
        &values,
        Some(42),
    );
}

#[test]
fn system_builds_for_manual_source() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime must be created");
    let _guard = runtime.enter();

    let instrument_config = representative_layout_configs()
        .into_iter()
        .last()
        .expect("representative layout config");
    let tuner_config = tuner_config_for(&instrument_config);
    let values = FineTunedValues::new();

    let (_net, _handle) = create_system::<f32>(
        2,
        ExcitementSource::Manual,
        &instrument_config,
        &tuner_config,
        &values,
        Some(42),
    );
}
