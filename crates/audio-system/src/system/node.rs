mod adsr_shape;
mod band;
mod controller;
mod formant;
mod generator;
mod handle;
mod pairing;

#[cfg(test)]
mod tests;

use std::{collections::HashMap, ops::Mul};

use adsr_shape::*;
use common::instrument::{BandChannel, BandConfig, Config as InstrumentConfig};
use fundsp::prelude::*;
use typenum::Unsigned;

pub use handle::*;

use crate::{
    grid::{RhythmGrid, RhythmGridEnvelope},
    node::{controller::NodeController, generator::NodeGenerator},
    values::FineTunedValues,
};

use super::grid::create_rhythm_grid_envelope;

/// Mounts bands of nodes based on the provided instrument configuration.
///
/// Each band has multiple nodes, according to its config and wired as follows:
///
/// ```text
/// Band (Left | Right)
/// ├── Node 0
/// │   └── NodeController
/// │       └── RhythmGridEnvelope
/// │           └── Generator
/// ├── Node 1
/// │   └── NodeController
/// │       └── RhythmGridEnvelope
/// │           └── Generator
/// └── Node N
///     └── NodeController
///         └── RhythmGridEnvelope
///             └── Generator
/// ```
///
pub fn mount_node_bands<S: Real + Float + 'static>(
    net: &mut Net,
    config: &InstrumentConfig,
    values: &FineTunedValues,
    excitement_source: NodeId,
    rhythm_data_source: NodeId,
) -> Vec<NodeHandle> {
    let (l_bands, r_bands) = config
        .0
        .iter()
        .partition::<Vec<_>, _>(|band| band.channel == BandChannel::Left);

    let (l_net, l_handles) = create_channel_bands::<S>(&l_bands, values);
    let (r_net, r_handles) = create_channel_bands::<S>(&r_bands, values);

    let handles = sort_inner_handles(l_handles, r_handles);

    let l_net = net.push(Box::new(l_net));
    let r_net = net.push(Box::new(r_net));

    net.pipe_output(l_net);
    net.pipe_output(r_net);

    connect_rhythm_to_channel_nets::<S>(net, rhythm_data_source, l_net, r_net);

    let pairings = build_excitement_pairings(config, l_net, r_net);
    connect_excitement_pairings(net, excitement_source, &pairings);

    handles.into_iter().map(|h| h.into_outer()).collect()
}

pub(self) fn sort_inner_handles(
    l_handles: Vec<InnerHandle>,
    r_handles: Vec<InnerHandle>,
) -> Vec<InnerHandle> {
    let mut v = Vec::from_iter(l_handles.into_iter().chain(r_handles));
    v.sort_by_key(|nh| nh.key);
    v
}

pub(self) fn connect_rhythm_to_channel_nets<S: Real + Float + 'static>(
    net: &mut Net,
    rhythm_data_source: NodeId,
    l_net: NodeId,
    r_net: NodeId,
) {
    let rhythm_data_len = <RhythmGrid<S> as AudioNode>::Outputs::USIZE;

    let lr_rhythm_split = net.push(Box::new(multisplit::<
        <RhythmGrid<S> as AudioNode>::Outputs,
        U2,
    >()));

    net.pipe_all(rhythm_data_source, lr_rhythm_split);

    for gi in 0..rhythm_data_len {
        net.connect(lr_rhythm_split, gi, l_net, gi);
        net.connect(lr_rhythm_split, gi + rhythm_data_len, r_net, gi);
    }
}

pub(self) fn build_excitement_pairings(
    config: &InstrumentConfig,
    l_net: NodeId,
    r_net: NodeId,
) -> Vec<pairing::ExcitementPairing> {
    config
        .0
        .iter()
        .enumerate()
        .flat_map(|(band_index, band_config)| {
            let net = match band_config.channel {
                BandChannel::Left => l_net,
                BandChannel::Right => r_net,
            };
            let prior_channel_nodes = config
                .0
                .iter()
                .take(band_index)
                .filter(|b| b.channel == band_config.channel)
                .map(|b| b.nodes.len())
                .sum::<usize>();
            let prior_global_nodes = config
                .0
                .iter()
                .take(band_index)
                .map(|b| b.nodes.len())
                .sum::<usize>();
            band_config
                .nodes
                .iter()
                .enumerate()
                .map(move |(node_index, n)| pairing::ExcitementPairing {
                    band_index,
                    net,
                    in_band_node_index: node_index,
                    prior_channel_nodes,
                    prior_global_nodes,
                    key: n.key,
                })
        })
        .collect()
}

pub(self) fn connect_excitement_pairings(
    net: &mut Net,
    excitement_source: NodeId,
    pairings: &[pairing::ExcitementPairing],
) {
    for pairing in pairings {
        net.connect(
            excitement_source,
            pairing.source_hit(),
            pairing.net,
            pairing.target_hit(),
        );
        net.connect(
            excitement_source,
            pairing.source_radius(),
            pairing.net,
            pairing.target_radius(),
        );
    }
}

pub(self) fn create_channel_bands<S: Real + Float + 'static>(
    bands: &[&BandConfig],
    values: &FineTunedValues,
) -> (Net, Vec<InnerHandle>) {
    let num_nodes = bands.iter().map(|b| b.nodes.len()).sum::<usize>();
    let rhythm_data_len = <RhythmGrid<S> as AudioNode>::Outputs::USIZE;
    let node_inputs_len = <NodeController<S> as AudioNode>::Inputs::USIZE;
    let mut net = Net::new(rhythm_data_len + num_nodes * node_inputs_len, bands.len());

    let split_grid_data = u_num_it::u_num_it!(
        [1, 2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
        match bands.len() {
            U => {
                type NumBands = NumType;

                net.push(Box::new(multisplit::<
                    <RhythmGrid<S> as AudioNode>::Outputs,
                    NumBands,
                >()))
            }
            _ => panic!("Unsupported number of bands: {}", bands.len()),
        }
    );

    for gi in 0..rhythm_data_len {
        net.connect_input(gi, split_grid_data, gi);
    }

    let handles =
        bands
            .iter()
            .enumerate()
            .fold(Vec::new(), |mut handles, (band_index, band_config)| {
                let handles_inner = Vec::from_iter(
                    band_config
                        .nodes
                        .iter()
                        .map(|node_config| InnerHandle::new(node_config.key, band_config.channel)),
                );

                let (_controllers_stack, _band, _rhythm_data_split) = u_num_it::u_num_it!(
                    [
                        1, 2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67,
                        71
                    ],
                    match band_config.nodes.len() {
                        U => {
                            type NumNodes = NumType;

                            let MountBandReturn {
                                controllers_stack,
                                band,
                                rhythm_data_split,
                            } = mount_band::<S, NumNodes>(
                                &mut net,
                                band_config,
                                &handles_inner,
                                band_index,
                                split_grid_data,
                                values,
                            );
                            handles.extend(handles_inner);

                            (controllers_stack, band, rhythm_data_split)
                        }
                        _ => panic!(
                            "Unsupported number of nodes in band: {}",
                            band_config.nodes.len()
                        ),
                    }
                );

                handles
            });

    net.check();

    (net, handles)
}

struct MountBandReturn {
    controllers_stack: NodeId,
    band: NodeId,
    rhythm_data_split: NodeId,
}

type EnvelopedNodeGenerator<S> = Pipe<
    RhythmGridEnvelope<S, NodeGenerator<S>, <NodeGenerator<S> as AudioNode>::Inputs>,
    SnoopBackend,
>;

pub(self) fn mount_band<S: Real + Float + 'static, N: Size<S> + Size<NodeController<S>>>(
    net: &mut Net,
    band_config: &BandConfig,
    handles: &[InnerHandle],
    band_index: usize,
    split_grid_data: NodeId,
    values: &FineTunedValues,
) -> MountBandReturn
where
    // Current NodeController arity is U2 -> U3.
    NodeController<S>: AudioNode<Inputs = U2, Outputs = U3>,
    U2: Mul<N>,
    <U2 as Mul<N>>::Output: Size<S>,
    U3: Mul<N>,
    <U3 as Mul<N>>::Output: Size<S>,
    // Current EnvelopedNodeGenerator arity is U7 -> U1.
    EnvelopedNodeGenerator<S>: AudioNode<Inputs = U7, Outputs = U1>,
    U7: Mul<N>,
    <U7 as Mul<N>>::Output: Size<S>,
    U1: Mul<N>,
    <U1 as Mul<N>>::Output: Size<S>,
    // Current NodeGenerator arity is U2 -> U1
    NodeGenerator<S>: AudioNode<Inputs = U2, Outputs = U1>,
    // Current RhythmGrid output U3
    RhythmGrid<S>: AudioNode<Outputs = U3>,
    U3: Mul<N>,
    <U3 as Mul<N>>::Output: Size<S>,
{
    let controllers_stack = create_controllers_stack::<S, N>(net, band_config, handles);
    let (band, num_band_outputs) =
        create_and_push_band_node::<S, N>(net, band_config, handles, values);

    let rhythm_data_split = net.push(Box::new(multisplit::<
        <RhythmGrid<S> as AudioNode>::Outputs,
        N,
    >()));

    let num_nodes = N::USIZE;
    let rhythm_data_len = <RhythmGrid<S> as AudioNode>::Outputs::USIZE;
    let node_ctrl_inputs_len = <NodeController<S> as AudioNode>::Inputs::USIZE;
    let node_ctrl_outputs_len = <NodeController<S> as AudioNode>::Outputs::USIZE;
    let envelope_node_gen_inputs_len = <EnvelopedNodeGenerator<S> as AudioNode>::Inputs::USIZE;

    // Map channel-net excitement inputs (after rhythm inputs) into controller-stack inputs.
    for ci in 0..(N::USIZE * <NodeController<S> as AudioNode>::Inputs::USIZE) {
        net.connect_input(rhythm_data_len + ci, controllers_stack, ci);
    }

    for rg_i in 0..rhythm_data_len {
        net.connect(
            split_grid_data,
            rg_i + band_index * rhythm_data_len,
            rhythm_data_split,
            rg_i,
        );
    }

    for node_i in 0..num_nodes {
        for rg_i in 0..rhythm_data_len {
            net.connect(
                rhythm_data_split,
                rg_i + node_i * rhythm_data_len,
                band,
                rg_i + node_i * envelope_node_gen_inputs_len,
            );
        }

        let band_node_input_base = node_i * envelope_node_gen_inputs_len;
        let ctrl_node_output_base = node_i * node_ctrl_outputs_len;

        // RhythmGridEnvelope layout per node:
        // [0..3): grid data, [3]: start ratio, [4]: duration ratio,
        // [5]: generator control, [6]: generator accent.
        net.connect(
            controllers_stack,
            ctrl_node_output_base,
            band,
            band_node_input_base + rhythm_data_len,
        );
        net.connect(
            controllers_stack,
            ctrl_node_output_base + 1,
            band,
            band_node_input_base + rhythm_data_len + 1,
        );
        net.connect(
            controllers_stack,
            ctrl_node_output_base + 2,
            band,
            band_node_input_base + rhythm_data_len + 3,
        );

        // Reuse the per-node hit-strength lane as generator control input.
        net.connect_input(
            rhythm_data_len + node_i * node_ctrl_inputs_len,
            band,
            band_node_input_base + rhythm_data_len + 2,
        );
    }

    for bi in 0..num_band_outputs {
        net.connect_output(band, bi, bi);
    }

    MountBandReturn {
        controllers_stack,
        band,
        rhythm_data_split,
    }
}

pub(self) fn create_controllers_stack<
    S: Real + Float + 'static,
    N: Size<S> + Size<NodeController<S>>,
>(
    net: &mut Net,
    band_config: &BandConfig,
    handles: &[InnerHandle],
) -> NodeId
where
    NodeController<S>: AudioNode<Inputs = U2, Outputs = U3>,
    U2: Mul<N>,
    <U2 as Mul<N>>::Output: Size<S>,
    U3: Mul<N>,
    <U3 as Mul<N>>::Output: Size<S>,
{
    net.push(Box::new(stacki::<N, _, _>(|i| {
        let node_config = band_config.nodes[i as usize];
        let handle = &handles[i as usize];

        let room_size_m3 = node_config.room_size_m3();
        let reverb_time_to_min60db = node_config.hr_bpm() as f64 / 60.0;

        (handle.take_excitement_snoop_hs() | handle.take_excitement_snoop_rad())
            >> controller::create_node_controller::<S>(
                node_config,
                &handle.accentuation,
                &handle.rhythm,
            )
            >> (reverb4_stereo(room_size_m3, reverb_time_to_min60db)
                | follow(reverb_time_to_min60db))
    })))
}

pub(self) fn create_and_push_band_node<
    S: Real + Float + 'static,
    N: Size<S> + Size<EnvelopedNodeGenerator<S>>,
>(
    net: &mut Net,
    band_config: &BandConfig,
    handles: &[InnerHandle],
    values: &FineTunedValues,
) -> (NodeId, usize)
where
    EnvelopedNodeGenerator<S>: AudioNode<Inputs = U7, Outputs = U1>,
    U7: Mul<N>,
    <U7 as Mul<N>>::Output: Size<S>,
    U1: Mul<N>,
    <U1 as Mul<N>>::Output: Size<S>,
    NodeGenerator<S>: AudioNode<Inputs = U2, Outputs = U1>,
{
    let h_set: HashMap<common::NodeKey, &InnerHandle> =
        HashMap::from_iter(handles.iter().map(|h| (h.key, h)));

    let an_band = band::create_band_node::<
        S,
        EnvelopedNodeGenerator<S>,
        N,
        <EnvelopedNodeGenerator<S> as AudioNode>::Inputs,
        <EnvelopedNodeGenerator<S> as AudioNode>::Outputs,
        _,
        _,
        _,
    >(
        band_config.clone(),
        |node_config| {
            let shape = adsr_shape_for_node::<S>(&node_config);
            let inner_handle = h_set
                .get(&node_config.key)
                .expect("inner handle for node key");

            create_rhythm_grid_envelope::<S, NodeGenerator<S>, U2>(
                generator::create_node_generator::<S>(&node_config, values),
                shape,
            ) >> inner_handle.take_output_snoop()
        },
        values,
    );

    let num_band_outputs = an_band.0.outputs();
    let band = net.push(Box::new(an_band));
    (band, num_band_outputs)
}
