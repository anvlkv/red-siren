use common::{
    instrument::{BandChannel, BandConfig, Config as InstrumentConfig, NodeConfig, Scale},
    NodeKey,
};
use fundsp::prelude::*;
use fundsp::typenum::Unsigned;
use insta::assert_debug_snapshot;
use insta_fun::prelude::*;

use super::{build_excitement_pairings, mount_node_bands, pairing, sort_inner_handles, NodeHandle};
use crate::{grid::RhythmGrid, node::controller::NodeController, values::FineTunedValues};

#[derive(Debug)]
struct PairingSample {
    key: (u8, u8),
    channel: &'static str,
    source_hit: usize,
    source_radius: usize,
    target_hit: usize,
    target_radius: usize,
}

fn rhythm_inputs_len() -> usize {
    <RhythmGrid<f32> as AudioNode>::Outputs::USIZE
}

fn excitement_inputs_per_node() -> usize {
    <NodeController<f32> as AudioNode>::Inputs::USIZE
}

fn total_excitement_inputs(config: &InstrumentConfig) -> usize {
    config.num_nodes_total() * excitement_inputs_per_node()
}

fn total_mount_inputs(config: &InstrumentConfig) -> usize {
    rhythm_inputs_len() + total_excitement_inputs(config)
}

fn make_node(key: NodeKey, frequency: f64) -> NodeConfig {
    NodeConfig {
        key,
        frequency,
        phase: 0.0,
        cents: 100.0,
        l_mm: 170.0,
        w_kg: 1.1,
        v_cm3: 10.0,
    }
}

fn make_test_config() -> InstrumentConfig {
    InstrumentConfig(
        vec![
            BandConfig {
                channel: BandChannel::Left,
                // Deliberately unsorted keys to verify mount sorting behavior.
                nodes: vec![make_node(NodeKey(0, 7), 220.0)],
            },
            BandConfig {
                channel: BandChannel::Right,
                nodes: vec![make_node(NodeKey(0, 3), 330.0)],
            },
        ],
        Scale::Yo,
    )
}

fn mounted_test_net() -> (InstrumentConfig, Net, Vec<NodeHandle>) {
    let config = make_test_config();
    let values = FineTunedValues::new();
    let mut net = Net::new(total_mount_inputs(&config), 2);

    let rhythm_data_source = net.push(Box::new(
        multipass::<<RhythmGrid<f32> as AudioNode>::Outputs>(),
    ));
    for i in 0..rhythm_inputs_len() {
        net.connect_input(i, rhythm_data_source, i);
    }

    let excitement_source = net.push(Box::new(multipass::<
        <typenum::U2 as std::ops::Mul<<NodeController<f32> as AudioNode>::Inputs>>::Output,
    >()));

    for i in 0..total_excitement_inputs(&config) {
        net.connect_input(rhythm_inputs_len() + i, excitement_source, i);
    }

    let handles = mount_node_bands::<f32>(
        &mut net,
        &config,
        &values,
        excitement_source,
        rhythm_data_source,
    );

    net.check();

    (config, net, handles)
}

fn channel_name(channel: BandChannel) -> &'static str {
    match channel {
        BandChannel::Left => "left",
        BandChannel::Right => "right",
    }
}

fn pairing_trace(config: &InstrumentConfig) -> Vec<PairingSample> {
    let mut probe = Net::new(0, 1);
    let left_net = probe.push(Box::new(dc(0.0)));
    let right_net = probe.push(Box::new(dc(0.0)));

    build_excitement_pairings(config, left_net, right_net)
        .into_iter()
        .map(|p| PairingSample {
            key: (p.key.band(), p.key.key()),
            channel: channel_name(config.channel_of_key(&p.key).expect("channel for key")),
            source_hit: p.source_hit(),
            source_radius: p.source_radius(),
            target_hit: p.target_hit(),
            target_radius: p.target_radius(),
        })
        .collect()
}

fn snapshot_config(num_samples: usize) -> SnapshotConfig {
    SnapshotConfigBuilder::default()
        .num_samples(num_samples)
        .chart_layout(Layout::SeparateChannels)
        .svg_width(640)
        .svg_height_per_channel(160)
        .with_inputs(true)
        .build()
        .unwrap()
}

fn input_constant_drive(config: &InstrumentConfig) -> InputSource {
    let rhythm_len = rhythm_inputs_len();
    let ctrl_inputs = excitement_inputs_per_node();
    let _ = config;

    InputSource::Generator(Box::new(move |_i, ch| {
        if ch < rhythm_len {
            match ch {
                0 => 1.0,  // trigger
                1 => 64.0, // ticks_per_beat
                _ => 0.0,  // ticks_to_next
            }
        } else {
            let lane = ch - rhythm_len;
            let in_node_lane = lane % ctrl_inputs;
            if in_node_lane == 0 {
                1.0 // hit
            } else {
                1.0 // radius
            }
        }
    }))
}

fn input_pulsed_drive(config: &InstrumentConfig) -> InputSource {
    let rhythm_len = rhythm_inputs_len();
    let ctrl_inputs = excitement_inputs_per_node();
    let _ = config;

    InputSource::Generator(Box::new(move |i, ch| {
        if ch < rhythm_len {
            match ch {
                0 => {
                    if i % 64 == 0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                1 => 64.0,
                _ => (63 - (i % 64)) as f32,
            }
        } else {
            let lane = ch - rhythm_len;
            let in_node_lane = lane % ctrl_inputs;
            if in_node_lane == 0 {
                if i % 64 == 0 {
                    1.0
                } else {
                    0.0
                }
            } else {
                1.0
            }
        }
    }))
}

fn input_channel_selective_drive(
    config: &InstrumentConfig,
    active_channel: BandChannel,
) -> InputSource {
    let rhythm_len = rhythm_inputs_len();
    let ctrl_inputs = excitement_inputs_per_node();
    let node_channels: Vec<BandChannel> = config
        .0
        .iter()
        .flat_map(|b| b.nodes.iter().map(move |_| b.channel))
        .collect();

    InputSource::Generator(Box::new(move |_i, ch| {
        if ch < rhythm_len {
            match ch {
                0 => 1.0,
                1 => 64.0,
                _ => 0.0,
            }
        } else {
            let lane = ch - rhythm_len;
            let node_index = lane / ctrl_inputs;
            let in_node_lane = lane % ctrl_inputs;
            let is_active = node_channels
                .get(node_index)
                .map(|c| *c == active_channel)
                .unwrap_or(false);

            if in_node_lane == 0 {
                if is_active {
                    1.0
                } else {
                    0.0
                }
            } else {
                1.0
            }
        }
    }))
}

fn input_fully_wired_mocked_drive(config: &InstrumentConfig) -> InputSource {
    let rhythm_len = rhythm_inputs_len();
    let ctrl_inputs = excitement_inputs_per_node();
    let node_count = std::cmp::Ord::max(config.num_nodes_total(), 1);

    // Fully mocked drive for the mounted net inputs:
    // - rhythm lanes: trigger + ticks_per_beat + ticks_to_next
    // - excitement lanes: per-node hit/radius with staggered patterns
    InputSource::Generator(Box::new(move |i, ch| {
        if ch < rhythm_len {
            let period = 96usize;
            let phase = i % period;
            match ch {
                0 => {
                    if phase == 0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                1 => 96.0,
                _ => (period - 1 - phase) as f32,
            }
        } else {
            let lane = ch - rhythm_len;
            let node_index = lane / ctrl_inputs;
            let in_node_lane = lane % ctrl_inputs;

            let base_period = 96usize;
            let node_offset = (node_index * 17) % base_period;
            let hit = if ((i + node_offset) % base_period) == 0 {
                1.0
            } else {
                0.0
            };

            let radius = 0.25
                + 0.75
                    * (((i + node_index * 31) % (base_period * node_count)) as f32
                        / (base_period * node_count) as f32);

            if in_node_lane == 0 {
                hit
            } else {
                radius
            }
        }
    }))
}

#[test]
fn excitement_pairing_index_math() {
    let mut net = Net::new(0, 1);
    let p = pairing::ExcitementPairing {
        net: net.push(Box::new(dc(0.0))),
        band_index: 1,
        in_band_node_index: 0,
        prior_channel_nodes: 1,
        prior_global_nodes: 2,
        key: NodeKey(1, 0),
    };

    // With NodeController Inputs=2 and RhythmGrid Outputs=3:
    // global node index = 2 -> source lanes [4, 5]
    // local node index in channel = 1 -> target lanes [5, 6]
    assert_eq!(p.source_hit(), 4);
    assert_eq!(p.source_radius(), 5);
    assert_eq!(p.target_hit(), 5);
    assert_eq!(p.target_radius(), 6);
}

#[test]
fn mount_node_bands_returns_sorted_handles() {
    let (_config, _net, handles) = mounted_test_net();

    let keys: Vec<NodeKey> = handles.iter().map(|h| h.key).collect();
    assert_eq!(keys, vec![NodeKey(0, 3), NodeKey(0, 7)]);
    assert_eq!(handles[0].channel, BandChannel::Right);
    assert_eq!(handles[1].channel, BandChannel::Left);
}

#[test]
fn sort_inner_handles_orders_by_key() {
    let left = vec![
        super::InnerHandle::new(NodeKey(0, 7), BandChannel::Left),
        super::InnerHandle::new(NodeKey(0, 1), BandChannel::Left),
    ];
    let right = vec![
        super::InnerHandle::new(NodeKey(1, 3), BandChannel::Right),
        super::InnerHandle::new(NodeKey(1, 2), BandChannel::Right),
    ];

    let sorted = sort_inner_handles(left, right);
    let keys: Vec<NodeKey> = sorted.iter().map(|h| h.key).collect();
    assert_eq!(
        keys,
        vec![NodeKey(0, 1), NodeKey(0, 7), NodeKey(1, 2), NodeKey(1, 3)]
    );
}

#[test]
fn mount_node_bands_wiring() {
    let (config, _net, handles) = mounted_test_net();
    let trace = pairing_trace(&config);

    let source_lanes = trace
        .iter()
        .flat_map(|row| [row.source_hit, row.source_radius])
        .collect::<Vec<_>>();
    assert_eq!(source_lanes, vec![0, 1, 2, 3]);

    let target_lanes = trace
        .iter()
        .flat_map(|row| [row.target_hit, row.target_radius])
        .collect::<Vec<_>>();
    assert_eq!(target_lanes, vec![3, 4, 3, 4]);

    let handle_channels = handles
        .iter()
        .map(|h| (h.key, channel_name(h.channel)))
        .collect::<Vec<_>>();
    assert_eq!(
        handle_channels,
        vec![(NodeKey(0, 3), "right"), (NodeKey(0, 7), "left")]
    );

    assert_debug_snapshot!("mount_node_bands_wiring", trace);
}

#[test]
fn mount_node_bands_audio_constant_drive_snapshot() {
    let (config, net, _handles) = mounted_test_net();

    assert_audio_unit_snapshot!(
        "mount_node_bands_audio_constant_drive",
        net,
        input_constant_drive(&config),
        snapshot_config(1024)
    );
}

#[test]
fn mount_node_bands_audio_pulsed_drive_snapshot() {
    let (config, net, _handles) = mounted_test_net();

    assert_audio_unit_snapshot!(
        "mount_node_bands_audio_pulsed_drive",
        net,
        input_pulsed_drive(&config),
        snapshot_config(2048)
    );
}

#[test]
fn mount_node_bands_audio_left_only_drive_snapshot() {
    let (config, net, _handles) = mounted_test_net();

    assert_audio_unit_snapshot!(
        "mount_node_bands_audio_left_only_drive",
        net,
        input_channel_selective_drive(&config, BandChannel::Left),
        snapshot_config(1024)
    );
}

#[test]
fn mount_node_bands_audio_right_only_drive_snapshot() {
    let (config, net, _handles) = mounted_test_net();

    assert_audio_unit_snapshot!(
        "mount_node_bands_audio_right_only_drive",
        net,
        input_channel_selective_drive(&config, BandChannel::Right),
        snapshot_config(1024)
    );
}

#[test]
fn mount_node_bands_audio_fully_wired_long_snapshot() {
    let (config, net, _handles) = mounted_test_net();

    assert_audio_unit_snapshot!(
        "mount_node_bands_audio_fully_wired_long",
        net,
        input_fully_wired_mocked_drive(&config),
        snapshot_config(8192)
    );
}
