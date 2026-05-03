use std::{marker::PhantomData, ops::Mul};

use common::instrument::NodeConfig;
use fundsp::prelude::*;

use crate::util::hash_str;

const FORMANT_ID: u64 = hash_str(concat!(module_path!(), "::Formant"));

const DEFAULT_Q: f32 = 3.7;
const MIN_Q: f32 = 0.001;
const MAX_Q: f32 = 10.0;
const SAFE_NYQUIST_FACTOR: f64 = 0.98;
const DEFAULT_SAMPLE_RATE: f64 = 44_100.0;
const TWO_PI: f64 = 2.0 * std::f64::consts::PI;
const MIN_FREQ_HZ: f64 = 1.0;
const HALF: f64 = 0.5;
const TWO: f64 = 2.0;
const ONE: f64 = 1.0;
const ZERO: f64 = 0.0;

#[derive(Clone)]
/// Formant filter that simulates the resonant frequencies of the vocal tract of a living being.
///
/// Inputs: 2
/// - audio signal
/// - Quality factor for the filter (higher values result in a narrower bandwidth around the formant frequency)
///
/// Outputs: 1
/// - Filtered audio signal
pub struct Formant<S: Real + Float> {
    nth: u8,
    frequency_hz: S,
    sample_rate_hz: S,
    x1: S,
    x2: S,
    y1: S,
    y2: S,
    b0: S,
    b1: S,
    b2: S,
    a1: S,
    a2: S,
    _sample_type: PhantomData<S>,
}

impl<S: Real + Float> Formant<S> {
    pub fn new(nth: u8, frequency_hz: S) -> Self {
        let mut node = Self {
            nth: std::cmp::max(nth, 1),
            frequency_hz,
            sample_rate_hz: convert::<f64, S>(DEFAULT_SAMPLE_RATE),
            x1: convert::<f64, S>(ZERO),
            x2: convert::<f64, S>(ZERO),
            y1: convert::<f64, S>(ZERO),
            y2: convert::<f64, S>(ZERO),
            b0: convert::<f64, S>(ZERO),
            b1: convert::<f64, S>(ZERO),
            b2: convert::<f64, S>(ZERO),
            a1: convert::<f64, S>(ZERO),
            a2: convert::<f64, S>(ZERO),
            _sample_type: PhantomData,
        };
        node.update_coefficients(DEFAULT_Q);
        node
    }

    pub fn from_node_config(config: &NodeConfig, nth: u8) -> Self {
        let clamped_nth = std::cmp::max(nth, 1);
        Self::new(
            clamped_nth,
            convert::<f64, S>(config.formant_hz(clamped_nth as usize)),
        )
    }

    fn effective_q(&self, q: f32) -> f32 {
        let q = q.clamp(MIN_Q, MAX_Q);
        let attenuation = (self.nth as f32).sqrt();
        (q / attenuation).clamp(MIN_Q, MAX_Q)
    }

    fn clamped_frequency(&self) -> S {
        let nyquist: S = self.sample_rate_hz * convert::<f64, S>(HALF * SAFE_NYQUIST_FACTOR);
        let min_freq: S = convert::<f64, S>(MIN_FREQ_HZ);
        let safe_nyquist = if nyquist > min_freq {
            nyquist
        } else {
            min_freq
        };
        let f = self.frequency_hz;
        if f < min_freq {
            min_freq
        } else if f > safe_nyquist {
            safe_nyquist
        } else {
            f
        }
    }

    fn update_coefficients(&mut self, q: f32) {
        let q_s: S = convert::<f32, S>(self.effective_q(q));
        let f = self.clamped_frequency();
        let two_pi: S = convert::<f64, S>(TWO_PI);
        let two: S = convert::<f64, S>(TWO);
        let one: S = convert::<f64, S>(ONE);
        let zero: S = convert::<f64, S>(ZERO);

        let w0: S = two_pi * (f / self.sample_rate_hz);
        let sin_w0 = w0.sin();
        let cos_w0 = w0.cos();
        let alpha: S = sin_w0 / (two * q_s);

        // RBJ band-pass (constant skirt gain) coefficients.
        let b0 = alpha;
        let b2 = -alpha;
        let a0 = one + alpha;
        let a1 = -(two * cos_w0);
        let a2 = one - alpha;

        let inv_a0 = one / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = zero;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }

    fn process_sample(&mut self, x0: S) -> S {
        let y0 = self.b0 * x0 + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x0;
        self.y2 = self.y1;
        self.y1 = y0;
        y0
    }
}

/// Create a single formant filter for the nth harmonic resonance of `config`.
pub fn create_formant<S: Real + Float>(config: &NodeConfig, nth: u8) -> An<Formant<S>> {
    An(Formant::from_node_config(config, nth))
}

type FormantBank<S, N> = Pipe<Pipe<MultiSplit<U2, N>, MultiStack<N, Formant<S>>>, Join<N>>;

/*
    let wet_dry_mixer: An<WDMixer> = multisplit::<U3, U2>()
        >> ((pass() | sink() | pass()) | (sink() | pass() | pass()))
        >> ((pass() * (constant(1.0) - pass())) | (pass() * pass()))
        >> pass() + pass();
*/
type WDMixer = Pipe<
    Pipe<
        Pipe<
            MultiSplit<U3, U2>,
            Stack<Stack<Stack<Pass, Sink<U1>>, Pass>, Stack<Stack<Sink<U1>, Pass>, Pass>>,
        >,
        Stack<
            Binop<FrameMul<U1>, Pass, Binop<FrameSub<U1>, Constant<U1>, Pass>>,
            Binop<FrameMul<U1>, Pass, Pass>,
        >,
    >,
    Binop<FrameAdd<U1>, Pass, Pass>,
>;

pub type FormantBankWD<S, N> = Pipe<
    Pipe<
        Stack<Split<U2>, MultiPass<U2>>,
        Stack<Stack<Pass, Pipe<MultiPass<U2>, FormantBank<S, N>>>, Pass>,
    >,
    WDMixer,
>;
/// Create a parallel bank of `num_formants` formant filters for `config` (F1..=Fnum_formants).
///
/// Inputs: 3
/// - audio signal
/// - Q factor for all filters
/// - wet/dry mix (`0.0` = fully dry, `1.0` = fully wet)
///
/// The output mix is `dry * (1.0 - wet_dry) + wet * wet_dry`.
pub fn create_formant_bank<S, N>(config: &NodeConfig) -> An<FormantBankWD<S, N>>
where
    S: Real + Float + 'static,
    N: Size<S> + Size<Formant<S>> + Size<f32>,
    U2: Mul<N>,
    <U2 as Mul<N>>::Output: Size<f32>,
    U1: Mul<N, Output = N>,
{
    let formant_bank: An<FormantBank<S, N>> = multisplit::<U2, N>()
        >> stacki::<N, Formant<S>, _>(|i| {
            let nth = (i + 1) as u8;
            create_formant::<S>(config, nth)
        })
        >> join::<N>();

    // The wet/dry mixer applies the wet/dry ratio to the combined output of the formant filters and mixes it with the dry signal.
    // [dry, wet, wet_dry] -> [mixed]
    let wet_dry_mixer: An<WDMixer> = multisplit::<U3, U2>()
        >> ((pass() | sink() | pass()) | (sink() | pass() | pass()))
        >> ((pass() * (constant(1.0) - pass())) | (pass() * pass()))
        >> pass() + pass();

    (split::<U2>() | multipass::<U2>())
        >> (pass() | (multipass::<U2>() >> formant_bank) | pass())
        >> wet_dry_mixer
}

impl<S: Real + Float> AudioNode for Formant<S> {
    const ID: u64 = FORMANT_ID;

    type Inputs = U2;

    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let audio: S = S::from_f32(input[0]);
        let q = input[1];
        self.update_coefficients(q);
        Frame::from([self.process_sample(audio).to_f32()])
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate_hz = convert::<f64, S>(sample_rate.max(MIN_FREQ_HZ));
        self.update_coefficients(DEFAULT_Q);
        self.reset();
    }

    fn allocate(&mut self) {}

    fn reset(&mut self) {
        let zero: S = convert::<f64, S>(ZERO);
        self.x1 = zero;
        self.x2 = zero;
        self.y1 = zero;
        self.y2 = zero;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    fn make_test_config() -> NodeConfig {
        NodeConfig::new_test_node(220.0)
    }

    fn snapshot_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .num_samples(num_samples)
            .chart_layout(Layout::CombinedPerChannelType)
            .svg_width(512)
            .svg_height_per_channel(128)
            .with_inputs(true)
            .input_title("Audio")
            .input_title("Q")
            .output_title("Filtered")
            .build()
            .unwrap()
    }

    fn bank_snapshot_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .num_samples(num_samples)
            .chart_layout(Layout::CombinedPerChannelType)
            .svg_width(512)
            .svg_height_per_channel(128)
            .with_inputs(true)
            .input_title("Audio")
            .input_title("Q")
            .input_title("Wet/Dry")
            .output_title("Mixed")
            .build()
            .unwrap()
    }

    fn impulse_input(len: usize, q: f32) -> InputSource {
        let audio: Vec<f32> = (0..len)
            .map(|i| if i == 0 { 1.0_f32 } else { 0.0 })
            .collect();
        let q_ch: Vec<f32> = vec![q; len];
        InputSource::VecByChannel(vec![audio, q_ch])
    }

    fn bank_impulse_input(len: usize, q: f32, wet_dry: f32) -> InputSource {
        let audio: Vec<f32> = (0..len)
            .map(|i| if i == 0 { 1.0_f32 } else { 0.0 })
            .collect();
        let q_ch: Vec<f32> = vec![q; len];
        let wet_dry_ch: Vec<f32> = vec![wet_dry; len];
        InputSource::VecByChannel(vec![audio, q_ch, wet_dry_ch])
    }

    #[test]
    fn formant_f1_impulse_response() {
        let config = make_test_config();
        let node = create_formant::<f32>(&config, 1);
        assert_audio_unit_snapshot!(
            "formant_f1_impulse_response",
            node,
            impulse_input(256, DEFAULT_Q),
            snapshot_config(256)
        );
    }

    #[test]
    fn formant_f5_impulse_response() {
        let config = make_test_config();
        let node = create_formant::<f32>(&config, 5);
        assert_audio_unit_snapshot!(
            "formant_f5_impulse_response",
            node,
            impulse_input(256, DEFAULT_Q),
            snapshot_config(256)
        );
    }

    #[test]
    fn formant_f1_varying_q() {
        let config = make_test_config();
        let node = create_formant::<f32>(&config, 1);
        let len = 256_usize;
        let audio: Vec<f32> = (0..len)
            .map(|i| if i == 0 { 1.0_f32 } else { 0.0 })
            .collect();
        // First half: narrow (high Q), second half: wide (low Q).
        let q_ch: Vec<f32> = (0..len)
            .map(|i| if i < len / 2 { MAX_Q } else { MIN_Q * 10.0 })
            .collect();
        assert_audio_unit_snapshot!(
            "formant_f1_varying_q",
            node,
            InputSource::VecByChannel(vec![audio, q_ch]),
            snapshot_config(len)
        );
    }

    #[test]
    fn formant_bank_5_impulse_response() {
        let config = make_test_config();
        let bank = create_formant_bank::<f32, U5>(&config);
        assert_audio_unit_snapshot!(
            "formant_bank_5_impulse_response",
            bank,
            bank_impulse_input(256, DEFAULT_Q, 0.5),
            bank_snapshot_config(256)
        );
    }

    #[test]
    fn formant_bank_5_impulse_response_dry_only() {
        let config = make_test_config();
        let bank = create_formant_bank::<f32, U5>(&config);
        assert_audio_unit_snapshot!(
            "formant_bank_5_impulse_response_dry_only",
            bank,
            bank_impulse_input(256, DEFAULT_Q, 0.0),
            bank_snapshot_config(256)
        );
    }
}
