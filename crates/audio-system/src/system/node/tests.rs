use common::{
    instrument::{BandChannel, BandConfig, Config as InstrumentConfig, NodeConfig, Scale},
    NodeKey,
};
use fundsp::prelude::*;
use fundsp::typenum::{Unsigned, U1};
use insta::assert_debug_snapshot;
use insta_fun::prelude::*;

use super::{
    build_excitement_pairings, create_and_push_band_node, create_channel_bands,
    create_controllers_stack, mount_band, mount_node_bands, pairing, sort_inner_handles,
    NodeHandle,
};
use crate::{grid::RhythmGrid, node::controller::NodeController, values::FineTunedValues};

#[derive(Debug)]
struct PairingSample {
    _key: (u8, u8),
    _channel: &'static str,
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

fn test_node_with_key(key: NodeKey, frequency: f64) -> NodeConfig {
    let mut node = NodeConfig::new_test_node(frequency);
    node.key = key;
    node
}

fn make_test_config() -> InstrumentConfig {
    InstrumentConfig(
        vec![
            BandConfig {
                channel: BandChannel::Left,
                // Deliberately unsorted keys to verify mount sorting behavior.
                nodes: vec![test_node_with_key(NodeKey(0, 7), 220.0)],
            },
            BandConfig {
                channel: BandChannel::Right,
                nodes: vec![test_node_with_key(NodeKey(0, 3), 330.0)],
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
            _key: (p.key.band(), p.key.key()),
            _channel: channel_name(config.channel_of_key(&p.key).expect("channel for key")),
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
        .warm_up(WarmUp::Seconds(0.25))
        .chart_layout(Layout::CombinedPerChannelType)
        .svg_width(640)
        .svg_height_per_channel(160)
        .with_inputs(true)
        .build()
        .unwrap()
}

fn wiring_drive_with_channels(num_channels: usize) -> InputSource {
    InputSource::Generator(Box::new(move |i, ch| {
        let period = 96usize;
        let phase = i % period;

        match ch {
            // RhythmGrid lanes: trigger, ticks_per_beat, ticks_to_next
            0 => {
                if phase == 0 {
                    1.0
                } else {
                    0.0
                }
            }
            1 => period as f32,
            2 => (period - 1 - phase) as f32,
            // Controller lanes (hit/radius) for per-node excitation inputs
            _ => {
                if num_channels <= 3 {
                    0.0
                } else {
                    let lane = ch - 3;
                    if lane % 2 == 0 {
                        if ((i + (lane / 2) * 17) % period) == 0 {
                            1.0
                        } else {
                            0.0
                        }
                    } else {
                        0.25 + 0.75 * ((phase as f32) / (period as f32))
                    }
                }
            }
        }
    }))
}

fn controller_drive() -> InputSource {
    // create_controllers_stack takes [hit_strength, radius]
    InputSource::Generator(Box::new(move |i, ch| {
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
            _ => 0.25 + 0.75 * (phase as f32 / period as f32),
        }
    }))
}

fn envelope_band_drive() -> InputSource {
    // create_and_push_band_node exposes one EnvelopedNodeGenerator input block:
    // [trigger, ticks_per_beat, ticks_to_next, start_ratio, duration_ratio, control, accent]
    InputSource::Generator(Box::new(move |i, ch| {
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
            1 => period as f32,
            2 => (period - 1 - phase) as f32,
            3 => 0.0,
            4 => 0.5,
            5 => 1.0,
            _ => 1.0,
        }
    }))
}

fn input_fully_wired_drive(config: &InstrumentConfig) -> InputSource {
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

fn make_single_node_band(channel: BandChannel, key: NodeKey, frequency: f64) -> BandConfig {
    BandConfig {
        channel,
        nodes: vec![test_node_with_key(key, frequency)],
    }
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
fn mount_node_bands_audio_fully_wired_long_snapshot() {
    let (config, net, _handles) = mounted_test_net();

    assert_audio_unit_snapshot!(
        "mount_node_bands_audio_fully_wired_long",
        net,
        input_fully_wired_drive(&config),
        snapshot_config(8192)
    );
}

#[test]
fn create_channel_bands_contract_for_single_band() {
    let values = FineTunedValues::new();
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 11), 220.0);
    let bands = vec![&band];

    let (_net, handles) = create_channel_bands::<f32>(&bands, &values);

    assert_eq!(handles.len(), 1);
    assert_eq!(handles[0].key, NodeKey(0, 11));
    assert_eq!(handles[0].channel, BandChannel::Left);
}

#[test]
fn create_controllers_stack_contract_for_single_node() {
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 12), 220.0);
    let handle = super::InnerHandle::new(NodeKey(0, 12), BandChannel::Left);
    let handles = vec![handle];

    let mut net = Net::new(2, 1);
    let controllers_stack = create_controllers_stack::<f32, U1>(&mut net, &band, &handles);

    net.connect_input(0, controllers_stack, 0);
    net.connect_input(1, controllers_stack, 1);
    net.connect_output(controllers_stack, 0, 0);
    net.check();
}

#[test]
fn create_controllers_stack_audio_snapshot() {
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 15), 220.0);
    let handle = super::InnerHandle::new(NodeKey(0, 15), BandChannel::Left);
    let handles = vec![handle];

    let mut net = Net::new(2, 1);
    let controllers_stack = create_controllers_stack::<f32, U1>(&mut net, &band, &handles);
    net.connect_input(0, controllers_stack, 0);
    net.connect_input(1, controllers_stack, 1);
    net.connect_output(controllers_stack, 0, 0);
    net.check();

    assert_audio_unit_snapshot!(
        "create_controllers_stack_single_node",
        net,
        controller_drive(),
        snapshot_config(2048)
    );
}

#[test]
fn create_and_push_band_node_contract_for_single_node() {
    let values = FineTunedValues::new();
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 13), 220.0);
    let handle = super::InnerHandle::new(NodeKey(0, 13), BandChannel::Left);
    let handles = vec![handle];

    let band_inputs = <super::EnvelopedNodeGenerator<f32> as AudioNode>::Inputs::USIZE;
    let mut net = Net::new(band_inputs, 1);

    let (band_node, num_band_outputs) =
        create_and_push_band_node::<f32, U1>(&mut net, &band, &handles, &values);

    for i in 0..band_inputs {
        net.connect_input(i, band_node, i);
    }
    net.connect_output(band_node, 0, 0);
    net.check();

    assert_eq!(num_band_outputs, 1);
}

#[test]
fn create_and_push_band_node_audio_snapshot() {
    let values = FineTunedValues::new();
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 16), 220.0);
    let handle = super::InnerHandle::new(NodeKey(0, 16), BandChannel::Left);
    let handles = vec![handle];

    let band_inputs = <super::EnvelopedNodeGenerator<f32> as AudioNode>::Inputs::USIZE;
    let mut net = Net::new(band_inputs, 1);

    let (band_node, _num_band_outputs) =
        create_and_push_band_node::<f32, U1>(&mut net, &band, &handles, &values);

    for i in 0..band_inputs {
        net.connect_input(i, band_node, i);
    }
    net.connect_output(band_node, 0, 0);
    net.check();

    assert_audio_unit_snapshot!(
        "create_and_push_band_node_single_node",
        net,
        envelope_band_drive(),
        snapshot_config(4096)
    );
}

#[test]
fn mount_band_contract_for_single_node() {
    let values = FineTunedValues::new();
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 14), 220.0);
    let handle = super::InnerHandle::new(NodeKey(0, 14), BandChannel::Left);
    let handles = vec![handle];

    let rhythm_len = rhythm_inputs_len();
    let ctrl_inputs = excitement_inputs_per_node();
    let mut net = Net::new(rhythm_len + ctrl_inputs, 1);

    let split_grid_data = net.push(Box::new(multisplit::<
        <RhythmGrid<f32> as AudioNode>::Outputs,
        U1,
    >()));
    for gi in 0..rhythm_len {
        net.connect_input(gi, split_grid_data, gi);
    }

    let mounted = mount_band::<f32, U1>(&mut net, &band, &handles, 0, split_grid_data, &values);
    net.check();

    assert_ne!(mounted.controllers_stack, mounted.band);
    assert_ne!(mounted.band, mounted.rhythm_data_split);
}

#[test]
fn mount_band_audio_snapshot() {
    let values = FineTunedValues::new();
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 17), 220.0);
    let handle = super::InnerHandle::new(NodeKey(0, 17), BandChannel::Left);
    let handles = vec![handle];

    let rhythm_len = rhythm_inputs_len();
    let ctrl_inputs = excitement_inputs_per_node();
    let total_inputs = rhythm_len + ctrl_inputs;
    let mut net = Net::new(total_inputs, 1);

    let split_grid_data = net.push(Box::new(multisplit::<
        <RhythmGrid<f32> as AudioNode>::Outputs,
        U1,
    >()));
    for gi in 0..rhythm_len {
        net.connect_input(gi, split_grid_data, gi);
    }

    let _mounted = mount_band::<f32, U1>(&mut net, &band, &handles, 0, split_grid_data, &values);
    net.check();

    assert_audio_unit_snapshot!(
        "mount_band_single_node",
        net,
        wiring_drive_with_channels(total_inputs),
        snapshot_config(4096)
    );
}

#[test]
fn create_channel_bands_audio_snapshot() {
    let values = FineTunedValues::new();
    let band = make_single_node_band(BandChannel::Left, NodeKey(0, 18), 220.0);
    let bands = vec![&band];

    let (net, _handles) = create_channel_bands::<f32>(&bands, &values);
    let total_inputs = rhythm_inputs_len() + excitement_inputs_per_node();

    assert_audio_unit_snapshot!(
        "create_channel_bands_single_band",
        net,
        wiring_drive_with_channels(total_inputs),
        snapshot_config(4096)
    );
}
