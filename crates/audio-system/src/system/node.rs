mod band;
mod controller;
mod formant;
mod generator;
mod pairing;

use std::ops::Mul;

use common::{
    instrument::{BandChannel, BandConfig, Config as InstrumentConfig, NodeConfig},
    NodeKey,
};
use fundsp::prelude::*;
use typenum::Unsigned;

use crate::{
    grid::{AdsrShape, RhythmGrid, RhythmGridEnvelope},
    node::{controller::NodeController, generator::NodeGenerator},
    values::FineTunedValues,
};

use super::grid::rhythm_grid_envelope;

pub struct NodeHandle {
    pub channel: BandChannel,
    pub key: NodeKey,
    pub accentuation: Shared,
    pub rhythm: Shared,
}

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

    let handles = {
        let mut v = Vec::from_iter(l_handles.into_iter().chain(r_handles));
        v.sort_by_key(|nh| nh.key);
        v
    };

    let l_net = net.push(Box::new(l_net));
    let r_net = net.push(Box::new(r_net));

    net.pipe_output(l_net);
    net.pipe_output(r_net);

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

    for pairing in config
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
            band_config
                .nodes
                .iter()
                .enumerate()
                .map(move |(node_index, n)| pairing::ExcitementPairing {
                    band_index,
                    net,
                    in_band_node_index: node_index,
                    prior_channel_nodes,
                    key: n.key,
                })
        })
    {
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

    handles
}

fn create_channel_bands<S: Real + Float + 'static>(
    bands: &[&BandConfig],
    values: &FineTunedValues,
) -> (Net, Vec<NodeHandle>) {
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
                let handles_inner =
                    Vec::from_iter(band_config.nodes.iter().map(|node_config| NodeHandle {
                        channel: band_config.channel,
                        key: node_config.key,
                        accentuation: shared(0.0),
                        rhythm: shared(0.0),
                    }));

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

type EnvelopedNodeGenerator<S> =
    RhythmGridEnvelope<S, NodeGenerator<S>, <NodeGenerator<S> as AudioNode>::Inputs>;

fn mount_band<S: Real + Float + 'static, N: Size<S> + Size<NodeController<S>>>(
    net: &mut Net,
    band_config: &BandConfig,
    handles: &[NodeHandle],
    band_index: usize,
    split_grid_data: NodeId,
    values: &FineTunedValues,
) -> MountBandReturn
where
    // Current NodeController arity is U2 -> U5.
    NodeController<S>: AudioNode<Inputs = U2, Outputs = U5>,
    U2: Mul<N>,
    <U2 as Mul<N>>::Output: Size<S>,
    U5: Mul<N>,
    <U5 as Mul<N>>::Output: Size<S>,
    // Current EnvelopedNodeGenerator arity is U9 -> U1.
    EnvelopedNodeGenerator<S>: AudioNode<Inputs = U9, Outputs = U1>,
    U9: Mul<N>,
    <U9 as Mul<N>>::Output: Size<S>,
    U1: Mul<N>,
    <U1 as Mul<N>>::Output: Size<S>,
    // Current NodeGenerator arity is U2 -> U1
    NodeGenerator<S>: AudioNode<Inputs = U2, Outputs = U1>,
    // Current RhythmGrid output U3
    RhythmGrid<S>: AudioNode<Outputs = U3>,
    U3: Mul<N>,
    <U3 as Mul<N>>::Output: Size<S>,
{
    let controllers_stack = net.push(Box::new(stacki::<N, _, _>(|i| {
        let node_config = band_config.nodes[i as usize];
        let handle = &handles[i as usize];
        let room_size_m3 = S::from_f64(node_config.v_cm3 / 1000.0); // Convert cm^3 to m^3 for reverb parameters.
        let time_to_min60db_s: S = S::from_f64(node_config.hr_bpm() as f64) / S::from_f64(60.0); // Time to decay to -60dB in seconds, scaled by tempo

        let reverb_unit = || reverb4_stereo(convert(room_size_m3), convert(time_to_min60db_s));

        controller::create_node_controller::<S>(node_config, &handle.accentuation, &handle.rhythm)
            >> (reverb_unit() | reverb_unit() | follow(time_to_min60db_s / S::from_f32(4.0)))
    })));

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
            rhythm_grid_envelope::<S, NodeGenerator<S>, U2>(
                generator::create_node_generator::<S>(&node_config, values),
                shape,
            )
        },
        values,
    );

    let num_band_outputs = an_band.0.outputs();

    let band = net.push(Box::new(an_band));

    let rhythm_data_split = net.push(Box::new(multisplit::<
        <RhythmGrid<S> as AudioNode>::Outputs,
        N,
    >()));

    let num_nodes = N::USIZE;
    let rhythm_data_len = <RhythmGrid<S> as AudioNode>::Outputs::USIZE;
    let node_ctrl_outputs_len = <NodeController<S> as AudioNode>::Outputs::USIZE;
    let envelope_node_gen_inputs_len = <EnvelopedNodeGenerator<S> as AudioNode>::Inputs::USIZE;

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
        for ctrl_i in 0..node_ctrl_outputs_len {
            net.connect(
                controllers_stack,
                ctrl_i + node_i * node_ctrl_outputs_len,
                band,
                rhythm_data_len + ctrl_i + node_i * envelope_node_gen_inputs_len,
            );
        }
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

fn adsr_shape_for_node<S: Real + Float + 'static>(node_config: &NodeConfig) -> AdsrShape<S> {
    // Generate ADSR shape from physical properties using time-constant model.
    // All computation in f64, convert outputs to S.

    // Time constant proxy from mass and displaced volume.
    let tau = (node_config.w_kg * node_config.v_cm3).sqrt();
    let tau_norm = (tau / 0.5).clamp(0.05, 2.0);

    // Attack/decay/release scale with time constant: lighter nodes are snappier.
    let attack = (0.08 * tau_norm).clamp(0.01, 0.25);

    let decay = (0.12 * tau_norm + 0.04).clamp(0.05, 0.3);

    // Sustain follows log-volume and anchors around 0.5 at 1 cm^3.
    let sustain = (0.5 + 0.12 * node_config.v_cm3.log10()).clamp(0.3, 0.9);

    let release = (0.14 * tau_norm + 0.08).clamp(0.1, 0.4);

    // Smoothness: linear mapping from τ to [0.01, 0.99]
    // Light nodes (sharp linear segments) → low smoothness
    // Heavy nodes (rounded curves) → high smoothness
    let smoothness = tau_norm.clamp(0.01, 0.99);

    // Convert outputs to S type
    AdsrShape {
        attack: S::from_f64(attack),
        decay: S::from_f64(decay),
        sustain: S::from_f64(sustain),
        release: S::from_f64(release),
        smoothness: S::from_f64(smoothness),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::instrument::Scale;
    use insta_fun::prelude::*;

    fn low_sr_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(100.0)
            .num_samples(num_samples)
            .build()
            .unwrap()
    }

    fn make_instrument_config() -> InstrumentConfig {
        InstrumentConfig(
            vec![
                BandConfig {
                    channel: BandChannel::Left,
                    nodes: vec![NodeConfig::new_test_node(220.0)],
                },
                BandConfig {
                    channel: BandChannel::Right,
                    nodes: vec![NodeConfig::new_test_node(880.0)],
                },
            ],
            Scale::Yo,
        )
    }

    #[test]
    fn mount_node_bands_wiring() {
        let config = make_instrument_config();
        let values = FineTunedValues::new();
        let mut net = Net::new(0, 2);
        // 3 outputs: grid trigger, ticks per beat, ticks to next beat.
        // Keep trigger high and tick spacing minimal so at least one event is visible in the snapshot.
        let rhythm_id = net.push(Box::new(dc(1.0) | dc(1.0) | dc(0.0)));
        // 4 outputs: 2 nodes × 2 controller inputs (hit + radius).
        // Non-zero inputs drive controller schedules so envelopes actually fire.
        let excitement_id = net.push(Box::new(dc(1.0) | dc(0.5) | dc(1.0) | dc(0.5)));

        let handles = mount_node_bands::<f32>(&mut net, &config, &values, excitement_id, rhythm_id);

        assert_eq!(handles.len(), 2, "should have one handle per node");
        assert_eq!(
            handles[0].channel,
            BandChannel::Left,
            "first handle is left channel"
        );
        assert_eq!(
            handles[1].channel,
            BandChannel::Right,
            "second handle is right channel"
        );

        assert_audio_unit_snapshot!(
            "mount_node_bands_wiring",
            net,
            InputSource::None,
            low_sr_config(512)
        );
    }

    #[test]
    fn test_adsr_light_node_flute() {
        // Light, small volume node (flute-like)
        let node = NodeConfig {
            key: NodeKey(0, 0),
            frequency: 2000.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 100.0,
            w_kg: 0.01,
            v_cm3: 0.1,
        };
        let shape = adsr_shape_for_node::<f32>(&node);

        // Light nodes should have:
        // - Fast attack (< 0.1)
        // - Short decay (< 0.15)
        // - Low sustain (< 0.5 due to small volume)
        // - Quick release (< 0.2)
        // - Low smoothness (< 0.2)
        assert!(
            shape.attack.to_f32() < 0.1,
            "attack should be fast for light node"
        );
        assert!(
            shape.decay.to_f32() < 0.15,
            "decay should be short for light node"
        );
        assert!(
            shape.sustain.to_f32() < 0.5,
            "sustain should be low for small volume"
        );
        assert!(
            shape.release.to_f32() < 0.2,
            "release should be quick for light node"
        );
        assert!(
            shape.smoothness.to_f32() < 0.2,
            "smoothness should be low for light node"
        );
    }

    #[test]
    fn test_adsr_medium_node_bell() {
        // Medium node (bell-like)
        let node = NodeConfig {
            key: NodeKey(0, 1),
            frequency: 500.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 170.0,
            w_kg: 0.1,
            v_cm3: 1.0,
        };
        let shape = adsr_shape_for_node::<f32>(&node);

        // Medium nodes should have balanced ADSR.
        assert!(shape.attack.to_f32() >= 0.05 && shape.attack.to_f32() <= 0.15);
        assert!(shape.decay.to_f32() >= 0.08 && shape.decay.to_f32() <= 0.2);
        assert!(shape.sustain.to_f32() >= 0.45 && shape.sustain.to_f32() <= 0.65);
        assert!(shape.release.to_f32() >= 0.15 && shape.release.to_f32() <= 0.3);
        assert!(shape.smoothness.to_f32() >= 0.4 && shape.smoothness.to_f32() <= 0.7);
    }

    #[test]
    fn test_adsr_heavy_node_gong() {
        // Heavy, large volume node (gong-like)
        let node = NodeConfig {
            key: NodeKey(0, 2),
            frequency: 200.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 250.0,
            w_kg: 1.0,
            v_cm3: 10.0,
        };
        let shape = adsr_shape_for_node::<f32>(&node);

        // Heavy nodes should have:
        // - Slower attack (> 0.1)
        // - Longer decay (> 0.2)
        // - Higher sustain (due to large volume)
        // - Long release (> 0.3)
        // - High smoothness (> 0.6)
        assert!(
            shape.attack.to_f32() > 0.1,
            "attack should be slower for heavy node"
        );
        assert!(
            shape.decay.to_f32() > 0.2,
            "decay should be longer for heavy node"
        );
        assert!(
            shape.sustain.to_f32() > 0.5,
            "sustain should be higher for large volume"
        );
        assert!(
            shape.release.to_f32() > 0.3,
            "release should be long for heavy node"
        );
        assert!(
            shape.smoothness.to_f32() > 0.6,
            "smoothness should be high for heavy node"
        );
    }

    #[test]
    fn test_adsr_very_light_edge_case() {
        // Very light node (edge case - should clamp properly)
        let node = NodeConfig {
            key: NodeKey(0, 3),
            frequency: 5000.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 50.0,
            w_kg: 0.001,
            v_cm3: 0.01,
        };
        let shape = adsr_shape_for_node::<f32>(&node);

        // Should be clamped to minimum values
        assert!(shape.attack.to_f32() >= 0.01, "attack minimum clamp");
        assert!(shape.decay.to_f32() >= 0.05, "decay minimum clamp");
        assert!(shape.sustain.to_f32() >= 0.3, "sustain minimum clamp");
        assert!(shape.release.to_f32() >= 0.1, "release minimum clamp");
        assert!(
            shape.smoothness.to_f32() >= 0.01,
            "smoothness minimum clamp"
        );
    }

    #[test]
    fn test_adsr_very_heavy_edge_case() {
        // Very heavy node (edge case - should clamp properly)
        let node = NodeConfig {
            key: NodeKey(0, 4),
            frequency: 100.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 300.0,
            w_kg: 10.0,
            v_cm3: 100.0,
        };
        let shape = adsr_shape_for_node::<f32>(&node);

        // Should be clamped to maximum values
        assert!(shape.attack.to_f32() <= 0.25, "attack maximum clamp");
        assert!(shape.decay.to_f32() <= 0.3, "decay maximum clamp");
        assert!(shape.sustain.to_f32() <= 0.9, "sustain maximum clamp");
        assert!(shape.release.to_f32() <= 0.4, "release maximum clamp");
        assert!(
            shape.smoothness.to_f32() <= 0.99,
            "smoothness maximum clamp"
        );
    }

    #[test]
    fn test_adsr_extreme_volume_small() {
        // Small volume node (extreme edge case)
        let node = NodeConfig {
            key: NodeKey(0, 5),
            frequency: 800.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 80.0,
            w_kg: 0.1,
            v_cm3: 0.001,
        };
        let shape = adsr_shape_for_node::<f32>(&node);

        // Very small volume should produce very low sustain
        assert!(
            shape.sustain.to_f32() < 0.35,
            "sustain should be minimal for tiny volume"
        );
    }

    #[test]
    fn test_adsr_f64_precision() {
        // Test with f64 precision
        let node = NodeConfig {
            key: NodeKey(0, 6),
            frequency: 440.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 170.0,
            w_kg: 0.1,
            v_cm3: 1.0,
        };
        let shape_f32 = adsr_shape_for_node::<f32>(&node);
        let shape_f64 = adsr_shape_for_node::<f64>(&node);

        // f32 and f64 should produce similar results (within tolerance)
        let tolerance = 0.001;
        assert!(
            (shape_f32.attack.to_f64() - shape_f64.attack).abs() < tolerance,
            "attack should be consistent across precision levels"
        );
        assert!(
            (shape_f32.decay.to_f64() - shape_f64.decay).abs() < tolerance,
            "decay should be consistent across precision levels"
        );
        assert!(
            (shape_f32.sustain.to_f64() - shape_f64.sustain).abs() < tolerance,
            "sustain should be consistent across precision levels"
        );
        assert!(
            (shape_f32.release.to_f64() - shape_f64.release).abs() < tolerance,
            "release should be consistent across precision levels"
        );
        assert!(
            (shape_f32.smoothness.to_f64() - shape_f64.smoothness).abs() < tolerance,
            "smoothness should be consistent across precision levels"
        );
    }
}
