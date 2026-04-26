mod band;
mod controller;
mod formant;
mod generator;

use common::{
    instrument::{BandChannel, BandConfig, Config as InstrumentConfig, NodeConfig},
    NodeKey,
};
use fundsp::prelude::*;
use typenum::Unsigned;

use crate::{
    grid::{AdsrShape, RhythmGrid},
    node::controller::NodeController,
    values::FineTunedValues,
};

use super::grid::rhythm_grid_envelope;

pub struct NodeHandle {
    key: NodeKey,
    accentuation: Shared,
    rhythm: Shared,
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
) {
    let (l_bands, r_bands) = config
        .0
        .iter()
        .cloned()
        .partition::<Vec<_>, _>(|band| band.channel == BandChannel::Left);

    // let mut handles = Vec::new();

    // // let (l_band_ids, r_band_ids) = []
    // let l_band_ids = [l_bands]
    //     // let controllers = band_config.nodes.iter().map(|node_config| {
    //     //     let handle = NodeHandle {
    //     //         key: node_config.key,
    //     //         accentuation: shared(0.0),
    //     //         rhythm: shared(0.0),
    //     //     };
    //     //     controller::create_node_controller(*node_config, &handle.accentuation, &handle.rhythm)
    //     // });
    // });
}

fn create_channel_bands<S: Real + Float + 'static>(
    bands: &[BandConfig],
    values: &FineTunedValues,
) -> (Net, Vec<NodeHandle>) {
    let num_nodes = bands.iter().map(|b| b.nodes.len()).sum::<usize>();

    let mut net = Net::new(
        <RhythmGrid<S> as AudioNode>::Outputs::USIZE
            + num_nodes * <NodeController<S> as AudioNode>::Inputs::USIZE,
        1,
    );

    let mut handles = Vec::new();

    let bands_ids = bands.iter().map(|band_config| {
        let handles_inner =
            Vec::from_iter(band_config.nodes.iter().map(|node_config| NodeHandle {
                key: node_config.key,
                accentuation: shared(0.0),
                rhythm: shared(0.0),
            }));

        let (controllers_stack, band) = u_num_it::u_num_it!(
            [1, 2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
            match band_config.nodes.len() {
                U => {
                    type NumNodes = NumType;

                    let ctrl_id =
                        net.push(Box::new(stacki::<NumNodes, NodeController<S>, _>(|i| {
                            let node_config = band_config.nodes[i as usize];
                            let handle = &handles_inner[i as usize];
                            let ctrl = controller::create_node_controller::<S>(
                                node_config,
                                &handle.accentuation,
                                &handle.rhythm,
                            );

                            ctrl
                        })));

                    let band_id = band::create_band_node::<S, _, NumNodes, _>(
                        band_config.clone(),
                        |node_config| {
                            let shape = adsr_shape_for_node::<S>(&node_config);
                            rhythm_grid_envelope::<S, _>(
                                generator::create_node_generator::<S>(&node_config, values),
                                shape,
                            )
                        },
                        values,
                    );

                    handles.extend(handles_inner);

                    (ctrl_id, band_id)
                }
                _ => panic!(
                    "Unsupported number of nodes in band: {}",
                    band_config.nodes.len()
                ),
            }
        );
    });

    (net, handles)
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
