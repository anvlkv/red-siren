use std::marker::PhantomData;

use common::instrument::NodeConfig;
use fundsp::prelude::*;

const CONTROLLER_ID: u64 = crate::util::hash_str("NodeController");
const NUM_LEVELS: usize = 11;

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
    const FIBONACCI: [u16; NUM_LEVELS] = [1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144];
    const HARMONIC_NUMERATORS: [u16; NUM_LEVELS] = [1, 9, 5, 4, 3, 5, 15, 2, 7, 11, 13];
    const HARMONIC_DENOMINATORS: [u16; NUM_LEVELS] = [1, 8, 4, 3, 2, 3, 8, 1, 4, 8, 8];

    fn map_index(drive: f64) -> usize {
        let max = (NUM_LEVELS - 1) as f64;
        let shaped = Self::shape_drive(drive);
        (shaped * max).round() as usize
    }

    fn shape_drive(drive: f64) -> f64 {
        // Keep control away from hard edges and smooth extremes.
        let x = drive.clamp(0.0, 1.0);
        let eased = x * x * (3.0 - 2.0 * x);
        0.08 + 0.84 * eased
    }

    fn bpm_drive(hr_bpm: u16) -> f64 {
        // Softly normalizes BPM to (0, 1), avoiding top-bucket saturation for high-but-common BPMs.
        let bpm = hr_bpm as f64;
        bpm / (bpm + 120.0)
    }

    fn density_drive(density_g_cm3: f64) -> f64 {
        // Maps (0, +inf) density to (0, 1) for stable duration coupling.
        density_g_cm3 / (density_g_cm3 + 1.0)
    }

    fn accent_bias(index: usize, accent: bool) -> usize {
        if accent {
            index.saturating_sub(1)
        } else {
            index
        }
    }

    fn fibonacci(index: usize) -> f32 {
        Self::FIBONACCI[index] as f32
    }

    fn harmonic_numerator(index: usize, accent: bool) -> f32 {
        Self::HARMONIC_NUMERATORS[Self::accent_bias(index, accent)] as f32
    }

    fn harmonic_denominator(index: usize, accent: bool) -> f32 {
        Self::HARMONIC_DENOMINATORS[Self::accent_bias(index, accent)] as f32
    }
}

impl<S: Float> AudioNode for NodeController<S> {
    const ID: u64 = CONTROLLER_ID;

    type Inputs = U2;
    type Outputs = typenum::op!(U2 + U2 + U1);

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let hit_strength = f64::from(input[0].clamp(0.0, 1.0));
        let radius = f64::from(input[1].clamp(0.0, 1.0));

        let mut frame = Frame::default();

        let accent = self.accentuation_control.value() > 0.5;
        let rhythm_value = f64::from(self.rhythm_control.value().clamp(-1.0, 1.0));
        let rhythm_u = (rhythm_value + 1.0) * 0.5;
        let bpm_drive = Self::bpm_drive(self.config.hr_bpm());
        let density_drive = Self::density_drive(self.config.body_density_g_cm3());

        frame[4] = if accent { 1.0 } else { 0.0 };

        if hit_strength != 0.0 {
            let start_divisor_drive =
                0.35 * bpm_drive + 0.30 * rhythm_u + 0.20 * radius + 0.15 * hit_strength;
            let start_divisor_index = Self::map_index(start_divisor_drive);

            let start_divisible_drive = 0.60 * rhythm_u + 0.40 * radius;
            let start_divisible_index = Self::map_index(start_divisible_drive);

            // Duration ties radius to body density, and also reacts to hit strength.
            let duration_physical_drive = radius * density_drive;
            let duration_divisor_drive =
                0.30 * bpm_drive + 0.25 * density_drive + 0.25 * radius + 0.20 * hit_strength;
            let duration_divisor_index = Self::map_index(duration_divisor_drive);

            let duration_divisible_drive =
                0.45 * duration_physical_drive + 0.35 * hit_strength + 0.20 * rhythm_u;
            let duration_divisible_index = Self::map_index(duration_divisible_drive);

            frame[0] = Self::fibonacci(start_divisible_index);
            frame[1] = Self::harmonic_denominator(start_divisor_index, accent);
            frame[2] = Self::harmonic_numerator(duration_divisible_index, accent);
            frame[3] = Self::fibonacci(duration_divisor_index);
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
            .chart_layout(Layout::CombinedPerChannelType)
            .svg_width(512)
            .svg_height_per_channel(128)
            .with_inputs(true)
            .input_title("Hit strength")
            .input_title("Radius")
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

    #[test]
    fn node_controller_invariants() {
        // Representative control sweep; avoids sampling-framework overhead.
        let hit_strengths = [0.1f32, 0.5, 1.0];
        let radii = [0.1f32, 0.5, 1.0];
        let accents = [0.0f32, 1.0];
        let rhythms = [-1.0f32, 0.0, 1.0];
        // A typical ticks-per-beat value (44 100 Hz / 120 BPM ≈ 368).
        // Using 100 keeps the arithmetic readable while still exercising the boundary.
        let ticks_per_beat: u64 = 100;

        for &accent in &accents {
            for &rhythm in &rhythms {
                let shared_accentuation = Shared::new(accent);
                let shared_rhythm = Shared::new(rhythm);
                let mut controller = create_node_controller::<f32>(
                    NodeConfig::new_test_node(440.0),
                    &shared_accentuation,
                    &shared_rhythm,
                );
                for &hit in &hit_strengths {
                    for &radius in &radii {
                        let mut input: Frame<f32, U2> = Frame::default();
                        input[0] = hit;
                        input[1] = radius;
                        let output = controller.tick(&input);

                        // Divisors must never be zero — compute_schedule divides by them.
                        assert!(
                            output[1] > 0.0,
                            "start divisor must be non-zero (accent={accent}, rhythm={rhythm}, hit={hit}, radius={radius})"
                        );
                        assert!(
                            output[3] > 0.0,
                            "duration divisor must be non-zero (accent={accent}, rhythm={rhythm}, hit={hit}, radius={radius})"
                        );
                        // Duration numerator must be non-zero to produce an audible event.
                        assert!(
                            output[2] > 0.0,
                            "duration divisible must be non-zero (accent={accent}, rhythm={rhythm}, hit={hit}, radius={radius})"
                        );

                        // Duration must resolve to at least one tick at ticks_per_beat.
                        let dur_div = output[2] as u64;
                        let dur_denom = output[3] as u64;
                        let total_ticks = (ticks_per_beat as f64 * dur_div as f64
                            / dur_denom as f64)
                            .round() as u64;
                        assert!(
                            total_ticks >= 1,
                            "duration must be at least 1 tick, got {total_ticks} \
                             (divisible={dur_div}, divisor={dur_denom}, accent={accent}, rhythm={rhythm})"
                        );

                        // Start offset must stay within a practical window.
                        // Worst case: Fibonacci[9]=89, harmonic_denominator[0]=1 → 89 beats.
                        let start_div = output[0] as u64;
                        let start_denom = output[1] as u64;
                        let start_ticks = (ticks_per_beat as f64 * start_div as f64
                            / start_denom as f64)
                            .round() as u64;
                        assert!(
                            start_ticks <= ticks_per_beat * 100,
                            "start offset must be within 100 beats, got {} beats \
                             (divisible={start_div}, divisor={start_denom}, accent={accent}, rhythm={rhythm})",
                            start_ticks / ticks_per_beat
                        );
                    }
                }
            }
        }
    }
}
