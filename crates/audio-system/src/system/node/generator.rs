use std::marker::PhantomData;

use common::instrument::NodeConfig;
use fundsp::prelude::*;

use crate::values::FineTunedValues;

const GENERATOR_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::NodeGenerator"));

#[derive(Clone)]
/// A node that generates audio signals based on a specified configuration.
///
/// Inputs: 2
/// - control signal (-1.0 to 1.0)
/// - accentuation on/off (1.0 for on, 0.0 for off)
///
/// Outputs: 1
/// - Generated audio signal
pub struct NodeGenerator<S: Real + Float + 'static> {
    inner: Box<dyn AudioUnit>,
    _sample_type: PhantomData<S>,
}

/// Create a [`NodeGenerator`] wrapped in [`An`] for use in FunDSP graphs.
pub fn create_node_generator<S: Real + Float + 'static>(
    config: &NodeConfig,
    values: &FineTunedValues,
) -> An<NodeGenerator<S>> {
    An(NodeGenerator::new(config, values))
}

impl<S: Real + Float + 'static> NodeGenerator<S> {
    pub fn new(config: &NodeConfig, values: &FineTunedValues) -> Self {
        let mut net = Net::new(2, 1);
        let freq: f32 = config.frequency as f32;
        let osc = net.push(Box::new(
            sine_hz::<S>(freq) | soft_saw_hz(freq) | saw_hz(freq),
        ));
        let src_3x_fade = net.push(Box::new(map(|frame: &Frame<f32, U4>| {
            let control_signal = frame[3];
            // -1.0 - full sine, 0.5 - no sine
            let sine_signal = frame[0];
            // 0.0 - full soft saw, 1.0 and -1.0 - no soft saw
            let soft_saw_signal = frame[1];
            // 1.0 - full saw, -0.5 - no saw
            let saw_signal = frame[2];

            let sine_mix = sine_signal * (1.0 - (control_signal + 1.0) / 1.5).max(0.0).min(1.0);
            let soft_saw_mix = soft_saw_signal * (1.0 - (control_signal.abs()));
            let saw_mix = saw_signal * ((control_signal + 0.5).max(0.0).min(1.0));

            sine_mix + soft_saw_mix + saw_mix
        })));

        let f_bank = net.push(Box::new(
            (multipass::<U2>() | values.formants_wet_dry_ratio.clone())
                >> super::formant::create_formant_bank::<S, U5>(config),
        ));
        let accent_input = net.push(Box::new(
            (pass() | values.formants_q.clone())
                >> map(|frame: &Frame<f32, U2>| {
                    let q_value = frame[0];
                    let accent_on = frame[1] > 0.5;

                    if accent_on {
                        q_value.powf(0.5) // Accentuation reduces the Q factor, making the formant filter more resonant
                    } else {
                        q_value
                    }
                }),
        ));
        net.connect_input(0, src_3x_fade, 3);
        net.connect_input(1, accent_input, 0);
        net.pipe_all(osc, src_3x_fade);

        net.connect(src_3x_fade, 0, f_bank, 0);
        net.connect(accent_input, 0, f_bank, 1);
        net.connect_output(f_bank, 0, 0);

        net.check();

        Self {
            inner: Box::new(net),
            _sample_type: PhantomData,
        }
    }
}

impl<S: Real + Float + 'static> AudioNode for NodeGenerator<S> {
    const ID: u64 = GENERATOR_ID;

    type Inputs = U2;
    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let mut output = Frame::default();
        self.inner.tick(input, &mut output);

        output
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        self.inner.process(size, input, output);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner.set_sample_rate(sample_rate);
    }

    fn allocate(&mut self) {
        self.inner.allocate();
    }

    fn set_hash(&mut self, hash: u64) {
        self.inner.set_hash(hash);
        self.reset();
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        chart_snapshot_config, constant_input_by_channel, linear_ramp_input,
    };
    use insta_fun::prelude::*;

    fn make_test_config() -> NodeConfig {
        NodeConfig::new_test_node(220.0)
    }

    fn snapshot_config(num_samples: usize) -> SnapshotConfig {
        chart_snapshot_config(num_samples, &["Control", "Accent"], &["Audio Out"])
    }

    fn steady_input(len: usize, control: f32, accent: f32) -> InputSource {
        constant_input_by_channel(len, &[control, accent])
    }

    fn control_sweep_input(len: usize) -> InputSource {
        let control = linear_ramp_input(-1.0, 1.0, len);
        InputSource::VecByChannel(vec![control, vec![0.0; len]])
    }

    #[test]
    fn node_generator_steady_unaccented() {
        let config = make_test_config();
        let values = FineTunedValues::new();
        let node = create_node_generator::<f32>(&config, &values);
        assert_audio_unit_snapshot!(
            "node_generator_steady_unaccented",
            node,
            steady_input(256, 0.0, 0.0),
            snapshot_config(256)
        );
    }

    #[test]
    fn node_generator_steady_accented() {
        let config = make_test_config();
        let values = FineTunedValues::new();
        let node = create_node_generator::<f32>(&config, &values);
        assert_audio_unit_snapshot!(
            "node_generator_steady_accented",
            node,
            steady_input(256, 0.0, 1.0),
            snapshot_config(256)
        );
    }

    #[test]
    fn node_generator_control_sweep() {
        let config = make_test_config();
        let values = FineTunedValues::new();
        let node = create_node_generator::<f32>(&config, &values);
        assert_audio_unit_snapshot!(
            "node_generator_control_sweep",
            node,
            control_sweep_input(256),
            snapshot_config(256)
        );
    }

    #[test]
    fn node_generator_30s_demo() {
        const SAMPLE_RATE: usize = 44100;
        const TOTAL: usize = 30 * SAMPLE_RATE;
        let config = make_test_config();
        let values = FineTunedValues::new();
        let node = create_node_generator::<f32>(&config, &values);
        // 0–10 s: control sweeps -1.0 → 1.0, accent off
        // 10–20 s: control sweeps 1.0 → -1.0, accent on
        // 20–30 s: control sweeps -1.0 → 1.0, accent alternates every 2 s
        let wav_cfg = SnapshotConfigBuilder::default()
            .num_samples(TOTAL)
            .output_mode(WavOutput::Wav32)
            .build()
            .unwrap();
        let input = || {
            InputSource::Generator(Box::new(move |i, ch| {
                let t = i as f32 / SAMPLE_RATE as f32;
                if ch == 0 {
                    // control signal
                    if t < 10.0 {
                        -1.0 + (t / 10.0) * 2.0
                    } else if t < 20.0 {
                        1.0 - ((t - 10.0) / 10.0) * 2.0
                    } else {
                        -1.0 + ((t - 20.0) / 10.0) * 2.0
                    }
                } else {
                    // accent channel
                    if t < 10.0 {
                        0.0
                    } else if t < 20.0 {
                        1.0
                    } else {
                        if ((t - 20.0) as usize % 2) == 0 {
                            0.0
                        } else {
                            1.0
                        }
                    }
                }
            }))
        };
        assert_audio_unit_snapshot!("node_generator_30s_demo", node.clone(), input(), wav_cfg);
        let chart_cfg = SnapshotConfigBuilder::default()
            .num_samples(3 * SAMPLE_RATE) // one second of each control phase
            .chart_layout(Layout::CombinedPerChannelType)
            .with_inputs(true)
            .input_title("Control")
            .input_title("Accent")
            .output_title("Audio Out")
            .build()
            .unwrap();

        assert_audio_unit_snapshot!("node_generator_30s_demo_chart", node, input(), chart_cfg);
    }
}
