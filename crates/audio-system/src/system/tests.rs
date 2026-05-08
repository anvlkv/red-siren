#![cfg(test)]

use std::cell::RefCell;
use std::rc::Rc;

use common::instrument::{
    config::{config_test_cases, representative_layout_configs},
    Config as InstrumentConfig, NodeConfig,
};
use common::tuner::{Config as TunerConfig, SensorData};
use fundsp::prelude::*;
use insta_fun::prelude::*;
use ordered_float::OrderedFloat;

use crate::rt::ExcitementSource;
use crate::system::{create_system, excitor::FFT_WINDOW_SIZE, values::FineTunedValues};

use super::grid::{adsr_shape_for_node, create_rhythm_grid, create_rhythm_grid_envelope};
use super::node::create_node_controller;

const DURATION_HARMONIC_NUMERATORS: [u16; 11] = [1, 9, 5, 4, 3, 5, 15, 2, 7, 11, 13];
const DURATION_FIBONACCI_DENOMINATORS: [u16; 11] = [1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144];

fn ratio_grid_wav_snapshot_config(num_samples: usize, sample_rate: f64) -> SnapshotConfig {
    SnapshotConfigBuilder::default()
        .sample_rate(sample_rate)
        .num_samples(num_samples)
        .output_assertion(OutputAssertion::Skip)
        .allow_abnormal_samples(true)
        .output_mode(WavOutput::Wav32)
        .build()
        .expect("snapshot config must be valid")
}

fn duration_lattice_ratios() -> Vec<f32> {
    let mut ratios = Vec::with_capacity(
        DURATION_HARMONIC_NUMERATORS.len() * DURATION_FIBONACCI_DENOMINATORS.len(),
    );
    for &den in &DURATION_FIBONACCI_DENOMINATORS {
        for &num in &DURATION_HARMONIC_NUMERATORS {
            ratios.push(num as f32 / den as f32);
        }
    }
    ratios
}

fn sequential_schedule_input(
    starts: Vec<f32>,
    durations: Vec<f32>,
    ticks_per_beat: usize,
) -> InputSource {
    InputSource::Generator(Box::new(move |i, ch| {
        if i > 0 && i % ticks_per_beat == 0 {
            let event_index = i / ticks_per_beat - 1;
            if event_index < durations.len() {
                if ch == 0 {
                    return starts[event_index];
                }
                if ch == 1 {
                    return durations[event_index];
                }
            }
        }
        f32::NAN
    }))
}

fn realistic_hit_radius_events() -> Vec<(f32, f32)> {
    let mut events = Vec::new();

    for step in 0..=16 {
        let x = step as f32 / 16.0;
        events.push((0.15 + 0.85 * x, 0.15 + 0.85 * x));
    }

    for step in 0..=16 {
        let x = step as f32 / 16.0;
        events.push((0.20 + 0.70 * x, 0.90 - 0.70 * x));
    }

    events.extend_from_slice(&[
        (1.0, 0.15),
        (0.85, 0.20),
        (0.65, 0.25),
        (0.45, 0.30),
        (0.30, 0.45),
        (0.20, 0.65),
        (0.15, 0.85),
        (0.10, 1.0),
    ]);

    events
}

fn beat_aligned_hit_radius_input(events: Vec<(f32, f32)>, ticks_per_beat: usize) -> InputSource {
    InputSource::Generator(Box::new(move |i, ch| {
        if i > 0 && i % ticks_per_beat == 0 {
            let event_index = i / ticks_per_beat - 1;
            if event_index < events.len() {
                let (hit, radius) = events[event_index];
                if ch == 0 {
                    return hit;
                }
                if ch == 1 {
                    return radius;
                }
            }
        }
        0.0
    }))
}

fn scheduler_grid_envelope_unit(bpm: f32) -> impl AudioUnit {
    let node_config = NodeConfig::new_test_node(220.0);
    let env = create_rhythm_grid_envelope::<f32, _, _>(
        sine_hz::<f32>(110.0),
        adsr_shape_for_node::<f32>(&node_config),
    );
    ((dc(bpm) >> create_rhythm_grid::<f32>()) | pass() | pass()) >> env
}

fn controller_grid_envelope_net(
    bpm: f32,
    accentuation: f32,
    rhythm: f32,
    node_config: NodeConfig,
) -> Net {
    let mut net = Net::new(2, 1);

    let bpm_src = net.push(Box::new(dc(bpm)));
    let grid = net.push(Box::new(create_rhythm_grid::<f32>()));

    let shared_accentuation = Shared::new(accentuation);
    let shared_rhythm = Shared::new(rhythm);
    let controller = net.push(Box::new(create_node_controller::<f32>(
        node_config.clone(),
        &shared_accentuation,
        &shared_rhythm,
    )));
    let env = net.push(Box::new(create_rhythm_grid_envelope::<f32, _, _>(
        sine_hz::<f32>(110.0),
        adsr_shape_for_node::<f32>(&node_config),
    )));

    net.connect(bpm_src, 0, grid, 0);

    net.connect_input(0, controller, 0);
    net.connect_input(1, controller, 1);

    net.connect(grid, 0, env, 0);
    net.connect(grid, 1, env, 1);
    net.connect(grid, 2, env, 2);
    net.connect(controller, 0, env, 3);
    net.connect(controller, 1, env, 4);

    net.pipe_output(env);
    net.check();
    net
}

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

#[test]
fn system_duration_ratio_lattice_grid_adsr_wav() {
    const SAMPLE_RATE: f64 = 2_000.0;
    const BPM: f32 = 240.0;

    let ticks_per_beat = (SAMPLE_RATE * 60.0 / BPM as f64).round() as usize;
    let durations = duration_lattice_ratios();
    let starts = vec![0.0; durations.len()];

    assert_eq!(durations.len(), 121);
    assert!(durations.iter().all(|d| d.is_finite() && *d > 0.0));

    let max_duration = durations.iter().copied().fold(0.0_f32, f32::max);
    let num_samples = (durations.len() + 2) * ticks_per_beat
        + (max_duration * ticks_per_beat as f32).ceil() as usize;

    let unit = scheduler_grid_envelope_unit(BPM);
    let input = sequential_schedule_input(starts, durations, ticks_per_beat);

    assert_audio_unit_snapshot!(
        "system_duration_ratio_lattice_grid_adsr_wav",
        unit,
        input,
        ratio_grid_wav_snapshot_config(num_samples, SAMPLE_RATE)
    );
}

#[test]
fn system_duration_ratio_controller_realistic_grid_adsr_wav() {
    const SAMPLE_RATE: f64 = 2_000.0;
    const BPM: f32 = 240.0;

    let ticks_per_beat = (SAMPLE_RATE * 60.0 / BPM as f64).round() as usize;
    let events = realistic_hit_radius_events();

    let distinct_pairs = events
        .iter()
        .map(|(hit, radius)| (OrderedFloat(*hit), OrderedFloat(*radius)))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        distinct_pairs.len() >= 20,
        "expected broad realistic hit/radius coverage"
    );

    let num_samples = (events.len() + 6) * ticks_per_beat;

    let unit = controller_grid_envelope_net(BPM, 0.0, 0.0, NodeConfig::new_test_node(220.0));
    let input = beat_aligned_hit_radius_input(events, ticks_per_beat);

    assert_audio_unit_snapshot!(
        "system_duration_ratio_controller_realistic_grid_adsr_wav",
        unit,
        input,
        ratio_grid_wav_snapshot_config(num_samples, SAMPLE_RATE)
    );
}
