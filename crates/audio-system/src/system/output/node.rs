use core::f64;
use std::{cell::RefCell, collections::HashMap, f32};

use common::{
    instrument::{GroupConfig, NodeConfig},
    NodeKey,
};
use fundsp::{
    hacker::prelude::*,
    typenum::{UInt, UTerm, B0, B1},
};

use super::formant::*;
use super::siren::*;
use super::InnerHandles;

use crate::system::values::{FineTunedValue, FineTunedValues};
use crate::util::S;

// Type alias for the siren excitement with follow envelope
type SirenExcitement = Pipe<Pipe<Var, Follow<S>>, SnoopBackend>;

type NonZeroControl = Pipe<Var, Shaper<ClipTo>>;

type DbLin = Pipe<FineTunedValue, super::db_lin::DbLinConverter>;

// Type alias for the siren alpha modulated by band control
type SirenAlpha = Binop<
    FrameMul<UInt<UTerm, B1>>,
    Pipe<
        Stack<
            Pipe<Stack<NonZeroControl, FineTunedValue>, super::div::Div<S>>,
            Unop<Var, FrameMulScalar<UInt<UTerm, B1>>>,
        >,
        super::pow::Pow<S>,
    >,
    NonZeroControl,
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
        Split<U2>,
        Stack<
            Pipe<
                Pipe<
                    Unop<FormantFilter, FrameMulScalar<UInt<UTerm, B1>>>,
                    Unop<FormantFilter, FrameMulScalar<UInt<UTerm, B1>>>,
                >,
                Unop<FormantFilter, FrameMulScalar<UInt<UTerm, B1>>>,
            >,
            Unop<Pass, FrameMulScalar<UInt<UTerm, B1>>>,
        >,
    >,
    Join<U2>,
>;

// Type alias for the bell filter with its 4 inputs
type BellFilter = Pipe<
    Stack<
        Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>,
        Binop<FrameAdd<U1>, DbLin, Var>,
    >,
    Svf<S, BellMode<S>>,
>;

// Complete node type composed from the smaller parts
pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<SirenWithInputs, Split<UInt<UInt<UTerm, B1>, B0>>>,
                Binop<FrameMul<UInt<UTerm, B1>>, SourceOscillator, Pass>,
            >,
            FormantBank,
        >,
        BellFilter,
    >,
    SnoopBackend,
>;

pub const ACTIVATION_SNOOP_CAPACITY: usize = 8;
pub const OUTPUT_SNOOP_CAPACITY: usize = 256;

fn create_node(
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
        >> (sine_phase::<S>(config.phase as f32) | saw() | An(band_control.clone()))
        >> super::crossfade::equal_power_crossfade();

    // Get the follow response time value
    #[cfg(feature = "editor")]
    let follow_time = node_follow_response_time_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = node_follow_response_time_s.value()[0];

    let siren_excitement: An<SirenExcitement> =
        An(siren_control) >> follow::<S>(follow_time as S) >> excitement_snoop;

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

    let formants: An<FormantBank> = split::<U2>()
        >> (((formant1 * 2.2) >> (formant2 * 0.6) >> (formant3 * 0.8)) | (pass() * 0.6))
        >> join::<U2>();

    let bell_filter: An<BellFilter> = (pass()
        | constant(config.frequency as f32)
        | node_bell_q
        | ((node_bell_gain_db >> super::db_lin::db_lin_converter()) + An(band_control.clone())))
        >> bell();

    siren_output >> split::<U2>() >> (source * pass()) >> formants >> bell_filter >> output_snoop
}

type ShelfType = Pipe<
    Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, DbLin>,
    Svf<S, LowshelfMode<S>>,
>;

pub type GroupType<K> = Pipe<Pipe<MultiBus<K, NodeType>, ShelfType>, ButterLowpass<S, U1>>;

pub fn create_group_node<K>(
    config: &GroupConfig,
    group_handles: HashMap<NodeKey, InnerHandles>,
    values: &FineTunedValues,
) -> An<GroupType<K>>
where
    K: Size<S> + Size<NodeType>,
{
    let nodes = config.nodes.clone();
    let handles_cell = RefCell::new(group_handles);
    let values_clone = values.clone();
    let f_min = config
        .nodes
        .iter()
        .map(|n| n.frequency)
        .fold(f64::MAX, |acc, x| acc.min(x)) as S;
    let f_max = config
        .nodes
        .iter()
        .map(|n| n.frequency)
        .fold(0_f64, |acc, x| acc.max(x)) as S;

    let shelf: An<ShelfType> = (pass()
        | constant((f_min * 0.86) as f32)
        | values.group_q.clone()
        | (values.group_ls_gain_db.clone() >> super::db_lin::db_lin_converter()))
        >> lowshelf::<S>();

    let butter = butterpass_hz(f_max * 3.2);

    busi::<K, _, _>(move |i| {
        let key = nodes[i as usize].key;
        let mut handles = handles_cell.borrow_mut();
        let handle = handles
            .remove(&key)
            .ok_or_else(|| format!("missing handle for node key: [{key:?}]"))
            .unwrap();

        create_node(&nodes[i as usize], handle, &values_clone)
    }) >> shelf
        >> butter
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;
    use test_log::test;

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
        use common::instrument::config_test_cases;

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let mut chart_config = SnapshotConfigBuilder::default();
        chart_config.warm_up(WarmUp::Samples(8000));
        chart_config.num_samples(4000);
        chart_config.show_grid(true);
        chart_config.chart_layout(Layout::Combined);

        for (config, layout) in config_test_cases() {
            let mut net = Net::new(0, config.0.iter().map(|g| g.nodes.len()).sum());
            let mut chart_config = chart_config.clone();

            chart_config.chart_title(format!(
                "{}x{}_{:?}",
                layout.space.x, layout.space.y, layout.scale
            ));

            for group in config.0 {
                for node in group.nodes {
                    let handles = InnerHandles::default();
                    handles.siren_control.set_value(0.25);
                    handles.band_control.set_value(0.25);
                    chart_config
                        .output_title(format!("node: {:?}; {}Hz", node.key, node.frequency));

                    let node = create_node(&node, handles, &values);
                    let id = net.push(Box::new(node));
                    net.pipe_output(id);
                }
            }

            let config = chart_config.build().unwrap();

            assert_audio_unit_snapshot!(net, config);
        }
    }

    #[test]
    fn test_group() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let nodes = vec![
            // silent
            NodeConfig::new_test_node(70.0),
            // 0.1
            NodeConfig::new_test_node(140.0),
            // 0.5
            NodeConfig::new_test_node(280.0),
            // 0.75
            NodeConfig::new_test_node(560.0),
            // 1.0
            NodeConfig::new_test_node(720.0),
        ];

        let group_handles = HashMap::from_iter(nodes.iter().enumerate().map(|(i, node)| {
            let handles = InnerHandles::default();
            match i {
                0 => handles.siren_control.set_value(0.0),
                1 => handles.siren_control.set_value(0.1),
                2 => handles.siren_control.set_value(0.5),
                3 => handles.siren_control.set_value(0.75),
                4 => handles.siren_control.set_value(1.0),
                _ => unreachable!(),
            }
            (node.key, handles)
        }));

        let config = GroupConfig {
            channel: common::instrument::GroupChannel::Left,
            nodes,
        };

        let node = create_group_node::<U5>(&config, group_handles, &values);

        let config = SnapshotConfigBuilder::default()
            .warm_up(WarmUp::Seconds(1.0))
            .num_samples(1500)
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(node, config);
    }
}
