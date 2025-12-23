use common::instrument::NodeConfig;
use fundsp::hacker::prelude::*;

use super::formant::*;
use super::siren::*;
use super::InnerHandles;

use crate::util::S;
use crate::ExcitementControl;
use crate::{
    system::values::{FineTunedValue, FineTunedValues},
    util::DbLin,
};

pub const ACTIVATION_SNOOP_CAPACITY: usize = 4;
pub const OUTPUT_SNOOP_CAPACITY: usize = 1024;

// Complete node type composed from the smaller parts
pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<Pipe<SirenWithInputs, Split<U2>>, Stack<SourceOscillator, Pass>>,
                Binop<FrameMul<U1>, FormantBank, Pass>,
            >,
            Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
        >,
        BellFilter,
    >,
    SnoopBackend,
>;

pub(super) fn create_node(
    config: &NodeConfig,
    handles: InnerHandles,
    values: &FineTunedValues,
) -> An<NodeType> {
    let InnerHandles {
        excitement_snoop,
        secondary_excitement_snoop,
        output_snoop,
        siren_control,
        band_control,
        siren_signum,
        ..
    } = handles;

    let FineTunedValues {
        node_follow_response_time_s,
        siren_alpha,
        formant_base_q,
        node_bell_q,
        node_bell_gain_db,
        ..
    } = values.clone();

    let source: An<SourceOscillator> =
        source_oscillator(config.frequency as S, config.phase as S, &band_control);

    let siren_excitement: An<SirenExcitement> = siren_excitement(
        &siren_control,
        &node_follow_response_time_s,
        excitement_snoop,
        secondary_excitement_snoop,
    );

    let modulated_alpha: An<SirenAlpha> = siren_modulated_alpha(&siren_alpha, &var(&band_control));

    // Stack inputs for siren
    let siren_output: An<SirenWithInputs> = siren_with_inputs(
        siren_excitement,
        modulated_alpha,
        An(siren_signum),
        config.divisions,
        config.cents,
        config.phase as S,
    );

    let formants: An<FormantBank> = formant_bank(config, &band_control, &formant_base_q);

    let bell_filter: An<BellFilter> = bell_filter(
        config.frequency as S,
        &node_bell_q,
        &node_bell_gain_db,
        &band_control,
    );

    siren_output
        >> split::<U2>()
        >> (source | pass())
        >> (formants * pass())
        >> mul(1.0 / config.divisions as f32)
        >> bell_filter
        >> output_snoop
}

// Type alias for the siren excitement with follow envelope
type SirenExcitement = Stack<
    Pipe<Pipe<Var, Pipe<Stack<Pass, Constant<U1>>, Hold>>, SnoopBackend>,
    Pipe<Pipe<Var, SnoopBackend>, Sink<U1>>,
>;

fn siren_excitement(
    control: &ExcitementControl,
    hold_time: &An<FineTunedValue>,
    excitement_snoop: An<SnoopBackend>,
    secondary_excitement_snoop: An<SnoopBackend>,
) -> An<SirenExcitement> {
    #[cfg(feature = "editor")]
    let hold_time = hold_time.value();
    #[cfg(not(feature = "editor"))]
    let hold_time = hold_time.value()[0];
    let hold_hz_val = 1.0 / hold_time;
    let hold_variability = 0.9;

    let primary =
        var(&control.primary) >> hold_hz(hold_hz_val, hold_variability) >> excitement_snoop;
    let secondary = var(&control.secondary) >> secondary_excitement_snoop >> sink();

    primary | secondary
}

type NonZeroControl<V = Var> = Pipe<V, Shaper<ClipTo>>;

// Type alias for the siren alpha modulated by band control
type SirenAlpha<V = Var> = Binop<
    FrameMul<U1>,
    Pipe<
        Stack<
            Pipe<Stack<NonZeroControl<V>, FineTunedValue>, super::div::Div<S>>,
            Unop<V, FrameMulScalar<U1>>,
        >,
        super::pow::Pow<S>,
    >,
    NonZeroControl<V>,
>;

fn siren_modulated_alpha<V>(siren_alpha: &An<FineTunedValue>, src_control: &V) -> An<SirenAlpha<V>>
where
    V: AudioNode<Outputs = U1, Inputs = U0>,
{
    let non_zero_control: An<NonZeroControl<V>> =
        An(src_control.clone()) >> clip_to(f32::EPSILON.sqrt(), 1.0);

    ((((non_zero_control.clone() | siren_alpha.clone()) >> super::div::div::<S>())
        | (An(src_control.clone()) * -1.0))
        >> super::pow::pow::<S>())
        * non_zero_control
}

// Type alias for the siren with all 5 inputs stacked
type SirenWithInputs<V = Var> = Pipe<
    Stack<
        Stack<Stack<Stack<SirenExcitement, SirenAlpha<V>>, Constant<U1>>, Constant<U1>>,
        Constant<U1>,
    >,
    super::siren::Siren<S>,
>;

#[allow(clippy::unnecessary_cast)]
fn siren_with_inputs<V>(
    siren_excitement: An<SirenExcitement>,
    modulated_alpha: An<SirenAlpha<V>>,
    siren_signum: An<Constant<U1>>,
    num_divisions: u32,
    cents: f64,
    phase: S,
) -> An<SirenWithInputs<V>>
where
    V: AudioNode<Outputs = U1, Inputs = U0>,
{
    let gamma: S = (1.0 - (1.0 / 1200.0) * (cents as S + S::EPSILON.sqrt())).abs();
    let beta: S = 1.0 / (num_divisions as S + gamma.sqrt());

    (siren_excitement
        | modulated_alpha
        | constant(beta as f32)
        | constant(gamma as f32)
        | siren_signum)
        >> siren_phase::<S>(phase)
}

// Type alias for a single formant filter with base_q input
type FormantFilter = Pipe<Stack<Stack<Pass, Var>, FineTunedValue>, super::formant::Formant>;

// Type alias for all three formant filters stacked with gain scaling
type FormantBank = Bus<
    Bus<
        Bus<Unop<FormantFilter, FrameMulScalar<U1>>, Unop<FormantFilter, FrameMulScalar<U1>>>,
        Unop<FormantFilter, FrameMulScalar<U1>>,
    >,
    Unop<Pass, FrameMulScalar<U1>>,
>;

fn formant_bank(
    config: &NodeConfig,
    band_control: &Shared,
    formant_base_q: &An<FineTunedValue>,
) -> An<FormantBank> {
    // Formants with base_q as input
    let formant1: An<FormantFilter> = (pass() | var(band_control) | formant_base_q.clone())
        >> formant(config.formant_hz(1) as S, 1);
    let formant2: An<FormantFilter> = (pass() | var(band_control) | formant_base_q.clone())
        >> formant(config.formant_hz(2) as S, 2);
    let formant3: An<FormantFilter> = (pass() | var(band_control) | formant_base_q.clone())
        >> formant(config.formant_hz(3) as S, 3);

    (formant1 * 1.45) & (formant2 * 1.33) & (formant3 * 1.22) & (pass() * 0.015)
}

// Type alias for the bell filter with its 4 inputs
type BellFilter = Pipe<
    Stack<
        Stack<
            Stack<Pass, Constant<U1>>,
            Binop<FrameMul<U1>, FineTunedValue, Binop<FrameAdd<U1>, Constant<U1>, Var>>,
        >,
        DbLin,
    >,
    Svf<S, BellMode<S>>,
>;

#[allow(clippy::unnecessary_cast)]
fn bell_filter(
    frequency: S,
    node_bell_q: &An<FineTunedValue>,
    node_bell_gain_db: &An<FineTunedValue>,
    band_control: &Shared,
) -> An<BellFilter> {
    (pass()
        | constant(frequency as f32)
        | (node_bell_q.clone() * (constant(1.0 / 3.0) + var(band_control)))
        | (node_bell_gain_db.clone() >> super::db_lin::db_lin_converter()))
        >> bell()
}

// Type alias for the source oscillator (abs >> clip >> sine/saw/control crossfade)
type SourceOscillator = Pipe<
    Pipe<
        Pipe<
            Binop<
                FrameMul<U1>,
                Pipe<Binop<FrameSub<U1>, Constant<U1>, super::abs::Abs>, Shaper<ClipTo>>,
                Constant<U1>,
            >,
            Split<U2>,
        >,
        Stack<Stack<Sine<S>, WaveSynth<U1>>, Var>,
    >,
    super::crossfade::EqualPowerCrossfade,
>;

#[allow(clippy::unnecessary_cast)]
fn source_oscillator(frequency: S, phase: S, band_control: &Shared) -> An<SourceOscillator> {
    let input = constant(1.0) - super::abs::abs();

    ((input >> clip_to(0.6, 1.0)) * constant(frequency as f32))
        >> split::<U2>()
        >> (sine_phase::<S>(phase as f32) | soft_saw() | var(band_control))
        >> super::crossfade::equal_power_crossfade()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::instrument::config_test_cases;
    use insta_fun::prelude::*;
    use test_log::test;

    #[test]
    fn test_alpha_modulation() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let src_control = shared(0.0);

        let modulated_alpha: An<SirenAlpha> =
            siren_modulated_alpha(&values.siren_alpha, &var(&src_control));

        let snapshot_config = SnapshotConfigBuilder::default()
            .num_samples(1000)
            .allow_abnormal_samples(true)
            .chart_layout(Layout::Combined)
            .output_title("Control")
            .output_title("Alpha")
            .build()
            .unwrap();

        let node = map(move |frame: &Frame<f32, U1>| {
            src_control.set_value(frame[0]);
            frame[0]
        }) | modulated_alpha;

        assert_audio_unit_snapshot!(
            "modulated_alpha",
            node,
            InputSource::Unit(Box::new(ramp_hz::<f32>(100.0))),
            snapshot_config
        );
    }

    #[test]
    fn test_siren_excitement() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let src_control = shared(0.0);

        let handles = InnerHandles::default();
        let excitement_control = ExcitementControl::new_primary(src_control.clone());
        let excitement: An<SirenExcitement> = siren_excitement(
            &excitement_control,
            &values.node_follow_response_time_s,
            handles.excitement_snoop,
            handles.secondary_excitement_snoop,
        );

        let snapshot_config = SnapshotConfigBuilder::default()
            .num_samples(1000)
            .allow_abnormal_samples(true)
            .chart_layout(Layout::Combined)
            .output_title("Control")
            .output_title("Excitement")
            .build()
            .unwrap();

        let node = map(move |frame: &Frame<f32, U1>| {
            src_control.set_value(frame[0]);
            frame[0]
        }) | excitement;

        assert_audio_unit_snapshot!(
            "siren_excitement",
            node,
            InputSource::Unit(Box::new(ramp_hz::<f32>(100.0))),
            snapshot_config
        );
    }

    #[test]
    fn test_siren_with_inputs() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let src_control = shared(0.0);

        let handles = InnerHandles::default();

        let excitement_control = ExcitementControl::new_primary(src_control.clone());
        let excitement: An<SirenExcitement> = siren_excitement(
            &excitement_control,
            &values.node_follow_response_time_s,
            handles.excitement_snoop,
            handles.secondary_excitement_snoop,
        );
        let modulated_alpha: An<SirenAlpha> =
            siren_modulated_alpha(&values.siren_alpha, &var(&src_control));
        let siren: An<SirenWithInputs> = siren_with_inputs(
            excitement,
            modulated_alpha,
            An(handles.siren_signum),
            4,
            50.0,
            0.0,
        );

        let snapshot_config = SnapshotConfigBuilder::default()
            .num_samples(1000)
            .allow_abnormal_samples(true)
            .chart_layout(Layout::Combined)
            .output_title("Control")
            .output_title("Siren output")
            .build()
            .unwrap();

        let node = map(move |frame: &Frame<f32, U1>| {
            src_control.set_value(frame[0]);
            frame[0]
        }) | siren;

        assert_audio_unit_snapshot!(
            "siren_with_inputs",
            node,
            InputSource::Unit(Box::new(ramp_hz::<f32>(100.0))),
            snapshot_config
        );
    }

    #[test]
    fn test_formant_bank() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let src_control = shared(0.0);

        let config = NodeConfig::new_test_node(440.0);

        let bank: An<FormantBank> = formant_bank(&config, &src_control, &values.formant_base_q);

        let snapshot_config = SnapshotConfigBuilder::default()
            .num_samples(1000)
            .allow_abnormal_samples(true)
            .chart_layout(Layout::Combined)
            .output_title("Control")
            .output_title("Output")
            .build()
            .unwrap();

        let node = map(move |frame: &Frame<f32, U1>| {
            src_control.set_value(frame[0]);
            frame[0]
        }) >> bank;

        assert_audio_unit_snapshot!(
            "formant_bank",
            node,
            InputSource::Unit(Box::new(ramp_hz::<f32>(100.0))),
            snapshot_config
        );
    }

    #[test]
    fn test_bell_filter() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let src_control = shared(0.0);

        let filter: An<BellFilter> = bell_filter(
            440.0 as S,
            &values.node_bell_q,
            &values.node_bell_gain_db,
            &src_control,
        );

        let snapshot_config = SnapshotConfigBuilder::default()
            .num_samples(1000)
            .allow_abnormal_samples(true)
            .with_inputs(true)
            .chart_layout(Layout::Combined)
            .build()
            .unwrap();

        let node = map(move |frame: &Frame<f32, U1>| {
            src_control.set_value(frame[0]);
            frame[0]
        }) >> filter;

        assert_audio_unit_snapshot!(
            "bell_filter",
            node,
            InputSource::Unit(Box::new(ramp_hz::<f32>(100.0))),
            snapshot_config
        );
    }

    #[test]
    fn test_source_oscillator() {
        let src_control = shared(0.0);

        let osc: An<SourceOscillator> = source_oscillator(440.0 as S, 0.0 as S, &src_control);

        let snapshot_config = SnapshotConfigBuilder::default()
            .num_samples(1000)
            .allow_abnormal_samples(true)
            .with_inputs(true)
            .chart_layout(Layout::Combined)
            .build()
            .unwrap();

        let node = map(move |frame: &Frame<f32, U1>| {
            src_control.set_value(frame[0]);
            frame[0]
        }) | osc;

        assert_audio_unit_snapshot!(
            "source_oscillator",
            node,
            InputSource::Unit(Box::new(ramp_hz::<f32>(100.0))),
            snapshot_config
        );
    }

    #[test]
    fn test_node() {
        let config = NodeConfig::new_test_node(440.0);

        let mut net = Net::new(0, 7);

        let mut chart_config = SnapshotConfigBuilder::default();
        chart_config.warm_up(WarmUp::Samples(8000));
        chart_config.num_samples(4000);
        chart_config.show_grid(true);
        chart_config.chart_layout(Layout::Combined);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        // silent
        let handles_1 = InnerHandles::default();
        let node_1 = create_node(&config, handles_1, &values);
        let id = net.push(Box::new(node_1));
        net.pipe_output(id);
        chart_config.output_title("silent");

        // siren control 0.1
        let handles_2 = InnerHandles::default();
        handles_2.siren_control.set_value((0.1, 0.0));
        let node_2 = create_node(&config, handles_2, &values);
        let id = net.push(Box::new(node_2));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.1");

        // siren control 0.1, band control 0.1
        let handles_3 = InnerHandles::default();
        handles_3.siren_control.set_value((0.1, 0.0));
        handles_3.band_control.set_value(0.1);
        let node_3 = create_node(&config, handles_3, &values);
        let id = net.push(Box::new(node_3));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.1, band control 0.1");

        // siren control 0.5
        let handles_4 = InnerHandles::default();
        handles_4.siren_control.set_value((0.5, 0.0));
        let node_4 = create_node(&config, handles_4, &values);
        let id = net.push(Box::new(node_4));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.5");

        // siren control 0.5, band control 0.5
        let handles_5 = InnerHandles::default();
        handles_5.siren_control.set_value((0.5, 0.0));
        handles_5.band_control.set_value(0.5);
        let node_5 = create_node(&config, handles_5, &values);
        let id = net.push(Box::new(node_5));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.5, band control 0.5");

        // siren control 1.0
        let handles_6 = InnerHandles::default();
        handles_6.siren_control.set_value((1.0, 0.0));
        let node_6 = create_node(&config, handles_6, &values);
        let id = net.push(Box::new(node_6));
        net.pipe_output(id);
        chart_config.output_title("siren control 1.0");

        // siren control 1.0, band control 1.0
        let handles_7 = InnerHandles::default();
        handles_7.siren_control.set_value((1.0, 0.0));
        handles_7.band_control.set_value(1.0);
        let node_7 = create_node(&config, handles_7, &values);
        let id = net.push(Box::new(node_7));
        net.pipe_output(id);
        chart_config.output_title("siren control 1.0, band control 1.0");

        let chart_config = chart_config.build().unwrap();

        assert_audio_unit_snapshot!(net, chart_config);
    }

    #[test]
    fn test_config_cases_nodes() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let mut snapshot_config = SnapshotConfigBuilder::default();
        snapshot_config.warm_up(WarmUp::Samples(8000));
        snapshot_config.num_samples(4000);
        let mut svg_config = SvgChartConfigBuilder::default();
        svg_config.show_grid(true);
        svg_config.chart_layout(Layout::Combined);
        svg_config.preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit());

        for (config, layout) in config_test_cases() {
            let mut net = Net::new(0, config.0.iter().map(|g| g.nodes.len()).sum());
            let mut snapshot_config = snapshot_config.clone();
            let mut svg_config = svg_config.clone();

            let case_title = format!(
                "config_test_case_node_{}x{}_{:?}",
                layout.space.x, layout.space.y, layout.scale
            );
            svg_config.chart_title(&case_title);

            for group in config.0 {
                for node in group.nodes {
                    let handles = InnerHandles::default();
                    handles.siren_control.set_value((0.25, 0.0));
                    handles.band_control.set_value(0.25);
                    let title = format!("node: {:?}; {}Hz", node.key, node.frequency);
                    svg_config.output_title(&title);

                    let node = create_node(&node, handles, &values);

                    let audio_snapshot_config = snapshot_config
                        .clone()
                        .output_mode(WavOutput::Wav32)
                        .num_samples(44100)
                        .build()
                        .unwrap();

                    assert_audio_unit_snapshot!(
                        format!("{case_title}-{title}"),
                        node.clone(),
                        InputSource::None,
                        audio_snapshot_config
                    );

                    let id = net.push(Box::new(node));
                    net.pipe_output(id);
                }
            }

            let config = snapshot_config
                .try_output_mode(svg_config)
                .unwrap()
                .build()
                .unwrap();

            assert_audio_unit_snapshot!(net, config);
        }
    }
}
