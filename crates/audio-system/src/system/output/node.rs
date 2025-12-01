use common::instrument::NodeConfig;
use fundsp::{
    hacker::prelude::*,
    typenum::{UInt, UTerm, B0, B1},
};

use super::formant::*;
use super::siren::*;
use super::InnerHandles;

use crate::util::S;
use crate::{
    system::values::{FineTunedValue, FineTunedValues},
    util::DbLin,
};

// Type alias for the siren excitement with follow envelope
type SirenExcitement = Pipe<Pipe<Var, Pipe<Stack<Pass, Constant<U1>>, Hold>>, SnoopBackend>;

type NonZeroControl<V = Var> = Pipe<V, Shaper<ClipTo>>;

// Type alias for the siren alpha modulated by band control
type SirenAlpha<V = Var> = Binop<
    FrameMul<UInt<UTerm, B1>>,
    Pipe<
        Stack<
            Pipe<Stack<NonZeroControl<V>, FineTunedValue>, super::div::Div<S>>,
            Unop<V, FrameMulScalar<UInt<UTerm, B1>>>,
        >,
        super::pow::Pow<S>,
    >,
    NonZeroControl<V>,
>;

// Type alias for the siren with all 5 inputs stacked
type SirenWithInputs = Pipe<
    Stack<
        Stack<Stack<Stack<SirenExcitement, SirenAlpha>, FineTunedValue>, FineTunedValue>,
        Constant<UInt<UTerm, B1>>,
    >,
    super::siren::Siren<S>,
>;

// Type alias for the source oscillator (abs >> clip >> sine/saw/control crossfade)
type SourceOscillator = Pipe<
    Pipe<
        Pipe<
            Binop<
                FrameMul<UInt<UTerm, B1>>,
                Pipe<super::abs::Abs, Shaper<ClipTo>>,
                Constant<UInt<UTerm, B1>>,
            >,
            Split<UInt<UInt<UTerm, B1>, B0>>,
        >,
        Stack<Stack<Sine<S>, WaveSynth<UInt<UTerm, B1>>>, Var>,
    >,
    super::crossfade::EqualPowerCrossfade,
>;

// Type alias for a single formant filter with base_q input
type FormantFilter = Pipe<Stack<Stack<Pass, Var>, FineTunedValue>, super::formant::Formant>;

// Type alias for all three formant filters stacked with gain scaling
type FormantBank = Pipe<
    Pipe<
        Split<U4>,
        Stack<
            Stack<
                Stack<
                    Unop<FormantFilter, FrameMulScalar<U1>>,
                    Unop<FormantFilter, FrameMulScalar<U1>>,
                >,
                Unop<FormantFilter, FrameMulScalar<U1>>,
            >,
            Unop<Pass, FrameMulScalar<U1>>,
        >,
    >,
    Join<U4>,
>;

// Type alias for the bell filter with its 4 inputs
type BellFilter = Pipe<
    Stack<
        Stack<
            Stack<Pass, Constant<UInt<UTerm, B1>>>,
            Binop<FrameMul<U1>, FineTunedValue, Binop<FrameAdd<U1>, Constant<U1>, Var>>,
        >,
        DbLin,
    >,
    Svf<S, BellMode<S>>,
>;

// Complete node type composed from the smaller parts
pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<SirenWithInputs, Split<UInt<UInt<UTerm, B1>, B0>>>,
                Stack<SourceOscillator, Pass>,
            >,
            Binop<FrameMul<UInt<UTerm, B1>>, FormantBank, Pass>,
        >,
        BellFilter,
    >,
    SnoopBackend,
>;

pub const ACTIVATION_SNOOP_CAPACITY: usize = 8;
pub const OUTPUT_SNOOP_CAPACITY: usize = 256;

pub(super) fn create_node(
    config: &NodeConfig,
    handles: InnerHandles,
    values: &FineTunedValues,
) -> An<NodeType> {
    let InnerHandles {
        excitement_snoop,
        output_snoop,
        siren_control,
        band_control,
        siren_signum,
        ..
    } = handles;

    let FineTunedValues {
        node_follow_response_time_s,
        siren_alpha,
        siren_beta,
        siren_gamma,
        formant_base_q,
        node_bell_q,
        node_bell_gain_db,
        ..
    } = values.clone();

    let source: An<SourceOscillator> = ((super::abs::abs() >> clip_to(0.85, 1.0))
        * constant(config.frequency as f32))
        >> split::<U2>()
        >> (sine_phase::<S>(config.phase as f32) | soft_saw() | An(band_control.clone()))
        >> super::crossfade::equal_power_crossfade();

    // Get the follow response time value
    #[cfg(feature = "editor")]
    let follow_time = node_follow_response_time_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = node_follow_response_time_s.value()[0];

    let siren_excitement: An<SirenExcitement> =
        An(siren_control) >> hold_hz(1.0 / follow_time, 0.3) >> excitement_snoop;

    let non_zero_control = An(band_control.clone()) >> clip_to(f32::EPSILON.sqrt(), 1.0);

    let modulated_alpha: An<SirenAlpha> = ((((non_zero_control.clone() | siren_alpha)
        >> super::div::div::<S>())
        | (An(band_control.clone()) * -1.0))
        >> super::pow::pow::<S>())
        * non_zero_control;

    // Stack inputs for siren
    let siren_output: An<SirenWithInputs> =
        (siren_excitement | modulated_alpha | siren_beta | siren_gamma | An(siren_signum))
            >> siren::<S>();

    // Formants with base_q as input
    let formant1: An<FormantFilter> = (pass() | An(band_control.clone()) | formant_base_q.clone())
        >> formant(config.formant_hz(1) as S, 1);
    let formant2: An<FormantFilter> = (pass() | An(band_control.clone()) | formant_base_q.clone())
        >> formant(config.formant_hz(2) as S, 2);
    let formant3: An<FormantFilter> = (pass() | An(band_control.clone()) | formant_base_q)
        >> formant(config.formant_hz(3) as S, 3);

    let formants: An<FormantBank> = split::<U4>()
        >> ((formant1 * 1.8) | (formant2 * 1.6) | (formant3 * 1.2) | (pass() * 0.002))
        >> join::<U4>();

    let bell_filter: An<BellFilter> = (pass()
        | constant(config.frequency as f32)
        | (node_bell_q * (constant(0.3) + An(band_control.clone())))
        | (node_bell_gain_db >> super::db_lin::db_lin_converter()))
        >> bell();

    siren_output
        >> split::<U2>()
        >> (source | pass())
        >> (formants * pass())
        >> bell_filter
        >> output_snoop
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

        let non_zero_control: An<NonZeroControl<Pass>> =
            pass() >> clip_to(f32::EPSILON.sqrt(), 1.0);

        let modulated_alpha: An<SirenAlpha<Pass>> =
            ((((non_zero_control.clone() | values.siren_alpha) >> crate::output::div::div::<S>())
                | (pass() * -1.0))
                >> crate::output::pow::pow::<S>())
                * non_zero_control;

        let snapshot_config = SnapshotConfigBuilder::default()
            .num_samples(1000)
            .allow_abnormal_samples(true)
            .with_inputs(true)
            .chart_layout(Layout::Combined)
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(
            "modulated_alpha",
            modulated_alpha,
            InputSource::Unit(Box::new(ramp_hz::<f32>(100.0) >> split::<U3>())),
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
        handles_2.siren_control.set_value(0.1);
        let node_2 = create_node(&config, handles_2, &values);
        let id = net.push(Box::new(node_2));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.1");

        // siren control 0.1, band control 0.1
        let handles_3 = InnerHandles::default();
        handles_3.siren_control.set_value(0.1);
        handles_3.band_control.set_value(0.1);
        let node_3 = create_node(&config, handles_3, &values);
        let id = net.push(Box::new(node_3));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.1, band control 0.1");

        // siren control 0.5
        let handles_4 = InnerHandles::default();
        handles_4.siren_control.set_value(0.5);
        let node_4 = create_node(&config, handles_4, &values);
        let id = net.push(Box::new(node_4));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.5");

        // siren control 0.5, band control 0.5
        let handles_5 = InnerHandles::default();
        handles_5.siren_control.set_value(0.5);
        handles_5.band_control.set_value(0.5);
        let node_5 = create_node(&config, handles_5, &values);
        let id = net.push(Box::new(node_5));
        net.pipe_output(id);
        chart_config.output_title("siren control 0.5, band control 0.5");

        // siren control 1.0
        let handles_6 = InnerHandles::default();
        handles_6.siren_control.set_value(1.0);
        let node_6 = create_node(&config, handles_6, &values);
        let id = net.push(Box::new(node_6));
        net.pipe_output(id);
        chart_config.output_title("siren control 1.0");

        // siren control 1.0, band control 1.0
        let handles_7 = InnerHandles::default();
        handles_7.siren_control.set_value(1.0);
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

        let mut chart_config = SnapshotConfigBuilder::default();
        chart_config.warm_up(WarmUp::Samples(8000));
        chart_config.num_samples(4000);
        let mut svg_config = SvgChartConfigBuilder::default();
        svg_config.show_grid(true);
        svg_config.chart_layout(Layout::Combined);
        svg_config.preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit());

        for (config, layout) in config_test_cases() {
            let mut net = Net::new(0, config.0.iter().map(|g| g.nodes.len()).sum());
            let mut chart_config = chart_config.clone();
            let mut svg_config = svg_config.clone();

            svg_config.chart_title(format!(
                "config_test_case_node_{}x{}_{:?}",
                layout.space.x, layout.space.y, layout.scale
            ));

            for group in config.0 {
                for node in group.nodes {
                    let handles = InnerHandles::default();
                    handles.siren_control.set_value(0.25);
                    handles.band_control.set_value(0.25);
                    svg_config.output_title(format!("node: {:?}; {}Hz", node.key, node.frequency));

                    let node = create_node(&node, handles, &values);
                    let id = net.push(Box::new(node));
                    net.pipe_output(id);
                }
            }

            let config = chart_config
                .try_output_mode(svg_config)
                .unwrap()
                .build()
                .unwrap();

            assert_audio_unit_snapshot!(net, config);
        }
    }
}
