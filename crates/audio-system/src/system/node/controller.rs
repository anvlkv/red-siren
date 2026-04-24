use std::marker::PhantomData;

use common::instrument::NodeConfig;
use fundsp::prelude::*;

const CONTROLLER_ID: u64 = crate::util::hash_str("NodeController");
const NUM_POWS: usize = 9; // Number of powers of 2 and 3 to precompute for rhythm scheduling

#[derive(Clone)]
/// Cntroller for a single node
///
/// Schedules the node's behavior based on its inputs and internal state, allowing for complex rhythmic patterns and accentuation.
///
/// ## Inputs: 2
/// - Hit strength (0.0 to 1.0)
/// - Radius (0.0 to 1.0)
///
/// ## Outputs: 5
///
/// ### Scheduler outputs in music time:
///
/// #### Scheduler start outputs (2):
/// - Divisible [0]
/// - Divisor [1]
///
/// #### Scheduler duration outputs (2):
/// - Divisible [2]
/// - Divisor [3]
///
/// ### Accentuation output (1):
/// - Accentuation value [4] (0.0 or 1.0)
pub struct NodeController<S: Float> {
    config: NodeConfig,
    /// Control for whether the current hit is accented or not. A hit is considered accented if the control value is above 0.5.
    ///
    /// `0.0 = false`
    /// `1.0 = true`
    accentuation_control: Var,
    /// Control for the rhythm value, which determines the timing of the node's behavior.
    ///
    /// `-1.0 = fastest rhythm (256th notes)`
    /// `0.0 = default rhythm (quarter notes)`
    /// `1.0 = slowest rhythm (whole notes)`
    rhythm_control: Var,
    _sample_type: PhantomData<S>,
}

impl<S: Float> NodeController<S> {
    const POWERS_OF_TWO: [u16; NUM_POWS] = [1, 2, 4, 8, 16, 32, 64, 128, 256];
    const POWERS_OF_THREE: [u16; NUM_POWS] = [1, 3, 9, 27, 81, 243, 729, 2187, 6561];
}

impl<S: Float> AudioNode for NodeController<S> {
    const ID: u64 = CONTROLLER_ID;

    type Inputs = U2;
    type Outputs = typenum::op!(U2 + U2 + U1);

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let hit_strength: S = convert(input[0].clamp(0.0, 1.0));
        let radius: S = convert(input[1].clamp(0.0, 1.0));

        let mut frame = Frame::default();

        let accent = self.accentuation_control.value() > 0.5;
        let rhythm_value: S = convert(self.rhythm_control.value().clamp(-1.0, 1.0));

        frame[4] = if accent { 1.0 } else { 0.0 };

        let num_pows = convert::<f32, S>(NUM_POWS as f32);
        if hit_strength != S::zero() {
            let start_divisor_index = (rhythm_value * (num_pows - convert(1.0)))
                .round()
                .to_i64()
                .unsigned_abs() as usize;
            let start_divisible_index = (rhythm_value * (num_pows * radius - convert(1.0)))
                .round()
                .to_i64()
                .unsigned_abs() as usize;

            let duration_divisor_index = (radius * (num_pows - convert(1.0)))
                .round()
                .to_i64()
                .unsigned_abs() as usize;
            let duration_divisible_index = (radius * (num_pows * hit_strength - convert(1.0)))
                .round()
                .to_i64()
                .unsigned_abs() as usize;

            if accent {
                // start divisible
                frame[0] = Self::POWERS_OF_THREE[start_divisible_index] as f32;
                // start divisor
                frame[1] = Self::POWERS_OF_THREE[start_divisor_index] as f32;
                // duration divisible
                frame[2] = convert::<S, f32>(
                    convert::<f32, S>(Self::POWERS_OF_THREE[duration_divisible_index] as f32)
                        / convert::<f32, S>(1000_f32)
                        * convert::<f64, S>(self.config.cents),
                );
                // duration divisor
                frame[3] = Self::POWERS_OF_THREE[duration_divisor_index] as f32;
            } else {
                // start divisible
                frame[0] = Self::POWERS_OF_TWO[start_divisible_index] as f32;
                // start divisor
                frame[1] = Self::POWERS_OF_TWO[start_divisor_index] as f32;
                // duration divisible
                frame[2] = convert::<S, f32>(
                    convert::<f32, S>(Self::POWERS_OF_TWO[duration_divisible_index] as f32)
                        / convert::<f32, S>(1000_f32)
                        * convert::<f64, S>(self.config.cents),
                );
                // duration divisor
                frame[3] = Self::POWERS_OF_TWO[duration_divisor_index] as f32;
            }
        }

        frame
    }
}

pub fn create_node_controller<S: Float>(
    config: NodeConfig,
    accentuation: &Shared,
    rhythm: &Shared,
) -> An<NodeController<S>> {
    let accentuation_control = Var::new(accentuation);
    let rhythm_control = Var::new(rhythm);

    An(NodeController {
        config,
        accentuation_control,
        rhythm_control,
        _sample_type: PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    fn input_data() -> InputSource {
        InputSource::VecByChannel(vec![
            vec![0.0, 1.0, 0.0, 0.5, 0.5, 0.0, 0.1, 0.0, 1.0], // hit strength
            vec![0.0, 0.5, 0.0, 0.1, 1.0, 0.0, 1.0, 0.0, 1.0], // radius
        ])
    }

    fn snapshot_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .num_samples(num_samples)
            .chart_layout(Layout::Combined)
            .svg_width(512)
            .svg_height_per_channel(128)
            .output_title("Start scheduler divisible")
            .output_title("Start scheduler divisor")
            .output_title("Duration scheduler divisible")
            .output_title("Duration scheduler divisor")
            .output_title("Accentuation")
            .build()
            .unwrap()
    }

    #[test]
    fn node_controller_unaccented() {
        let shared_accentuation = Shared::new(0.0);
        let shared_rhythm = Shared::new(0.0);
        let controller = create_node_controller::<f32>(
            NodeConfig::new_test_node(440.0),
            &shared_accentuation,
            &shared_rhythm,
        );

        assert_audio_unit_snapshot!(
            "node_controller_unaccented",
            controller,
            input_data(),
            snapshot_config(9)
        );
    }

    #[test]
    fn node_controller_accented() {
        let shared_accentuation = Shared::new(1.0);
        let shared_rhythm = Shared::new(0.0);
        let controller = create_node_controller::<f32>(
            NodeConfig::new_test_node(440.0),
            &shared_accentuation,
            &shared_rhythm,
        );

        assert_audio_unit_snapshot!(
            "node_controller_accented",
            controller,
            input_data(),
            snapshot_config(9)
        );
    }

    #[test]
    fn node_controller_accented_rhythm_plus() {
        let shared_accentuation = Shared::new(1.0);
        let shared_rhythm = Shared::new(1.0);
        let controller = create_node_controller::<f32>(
            NodeConfig::new_test_node(440.0),
            &shared_accentuation,
            &shared_rhythm,
        );

        assert_audio_unit_snapshot!(
            "node_controller_accented_rhythm_plus",
            controller,
            input_data(),
            snapshot_config(9)
        );
    }

    #[test]
    fn node_controller_accented_rhythm_minus() {
        let shared_accentuation = Shared::new(1.0);
        let shared_rhythm = Shared::new(-1.0);
        let controller = create_node_controller::<f32>(
            NodeConfig::new_test_node(440.0),
            &shared_accentuation,
            &shared_rhythm,
        );

        assert_audio_unit_snapshot!(
            "node_controller_accented_rhythm_minus",
            controller,
            input_data(),
            snapshot_config(9)
        );
    }

    #[test]
    fn node_controller_unaccented_rhythm_plus() {
        let shared_accentuation = Shared::new(0.0);
        let shared_rhythm = Shared::new(1.0);
        let controller = create_node_controller::<f32>(
            NodeConfig::new_test_node(440.0),
            &shared_accentuation,
            &shared_rhythm,
        );

        assert_audio_unit_snapshot!(
            "node_controller_unaccented_rhythm_plus",
            controller,
            input_data(),
            snapshot_config(9)
        );
    }

    #[test]
    fn node_controller_unaccented_rhythm_minus() {
        let shared_accentuation = Shared::new(0.0);
        let shared_rhythm = Shared::new(-1.0);
        let controller = create_node_controller::<f32>(
            NodeConfig::new_test_node(440.0),
            &shared_accentuation,
            &shared_rhythm,
        );

        assert_audio_unit_snapshot!(
            "node_controller_unaccented_rhythm_minus",
            controller,
            input_data(),
            snapshot_config(9)
        );
    }
}
