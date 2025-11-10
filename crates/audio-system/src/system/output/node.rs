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

// Type alias for the siren with all 5 inputs stacked
type SirenWithInputs = Pipe<
    Stack<
        Stack<
            Stack<
                SirenExcitement,
                Binop<FrameMul<UInt<UTerm, B1>>, FineTunedValue, Pipe<Var, Shaper<ClipTo>>>,
            >,
            FineTunedValue,
        >,
        FineTunedValue,
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
type FormantFilter<const N: u8> =
    Pipe<Stack<Stack<Pass, Var>, FineTunedValue>, super::formant::Formant<N>>;

// Type alias for all three formant filters stacked with gain scaling
type FormantBank = Pipe<
    Pipe<
        Split<U3>,
        Stack<
            Stack<
                Unop<FormantFilter<1>, FrameMulScalar<UInt<UTerm, B1>>>,
                Unop<FormantFilter<2>, FrameMulScalar<UInt<UTerm, B1>>>,
            >,
            Unop<FormantFilter<3>, FrameMulScalar<UInt<UTerm, B1>>>,
        >,
    >,
    Join<U3>,
>;

// Type alias for the bell filter with its 4 inputs
type BellFilter = Pipe<
    Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, FineTunedValue>,
    Svf<S, BellMode<S>>,
>;

// Complete node type composed from the smaller parts
pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<
                    Pipe<SirenWithInputs, Split<UInt<UInt<UTerm, B1>, B0>>>,
                    Binop<FrameMul<UInt<UTerm, B1>>, SourceOscillator, Pass>,
                >,
                FormantBank,
            >,
            super::chorus::Chorus,
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

    let source: An<SourceOscillator> = ((super::abs::abs() >> clip_to(0.95, 1.0))
        * constant(config.base_frequency as f32))
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

    // Stack inputs for siren (5 inputs total: excitement + 4 fine-tuned values)
    let siren_output: An<SirenWithInputs> = (siren_excitement
        | (siren_alpha * (An(band_control.clone()) >> clip_to(f32::EPSILON.sqrt(), 1.0)))
        | siren_beta
        | siren_gamma)
        >> siren::<S>();

    // Formants with base_q as input
    let formant1: An<FormantFilter<1>> =
        (pass() | An(band_control.clone()) | formant_base_q.clone())
            >> formant::<1>(config.base_frequency as S, config.divisions);
    let formant2: An<FormantFilter<2>> =
        (pass() | An(band_control.clone()) | formant_base_q.clone())
            >> formant::<2>(config.base_frequency as S, config.divisions);
    let formant3: An<FormantFilter<3>> = (pass() | An(band_control.clone()) | formant_base_q)
        >> formant::<3>(config.base_frequency as S, config.divisions);

    let formants: An<FormantBank> =
        split::<U3>() >> ((formant1 * 1.0) | (formant2 * 0.8) | (formant3 * 0.6)) >> join::<U3>();

    let bell_filter: An<BellFilter> =
        (pass() | constant(config.base_frequency as f32) | node_bell_q | node_bell_gain_db)
            >> bell();

    siren_output
        >> split::<U2>()
        >> (source * pass())
        >> formants
        >> super::chorus::chorus(config.key.idx() as u64, 0.0015, 0.0075, 1.75)
        >> bell_filter
        >> output_snoop
}

type ShelfType = Pipe<
    Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, FineTunedValue>,
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
    let first_node = config.nodes.first().unwrap();
    let last_node = config.nodes.last().unwrap();

    let shelf: An<ShelfType> = (pass()
        | constant(first_node.base_frequency as f32)
        | values.group_q.clone()
        | values.group_ls_gain.clone())
        >> lowshelf::<S>();

    let butter = butterpass_hz(last_node.base_frequency as S * 1.75);

    busi::<K, _, _>(move |i| {
        let key = nodes[i as usize].key;
        let mut handles = handles_cell.borrow_mut();
        let handle = handles.remove(&key).expect("missing handle for node key");
        create_node(&nodes[i as usize], handle, &values_clone)
    }) >> shelf
        >> butter
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_node() {
        let config = NodeConfig {
            key: NodeKey(0, 0),
            base_frequency: 440.0,
            phase: 0.0,
            divisions: 3,
        };

        let mut chart_config = SnapshotConfigBuilder::default();
        chart_config.num_samples(4000);
        chart_config.show_grid(true);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        // silent
        let handles_1 = InnerHandles::default();
        let node_1 = create_node(&config, handles_1, &values);

        chart_config.output_title("silent");

        // siren control 0.1
        let handles_2 = InnerHandles::default();
        handles_2.siren_control.set_value(0.1);
        let node_2 = create_node(&config, handles_2, &values);

        chart_config.output_title("siren control 0.1");

        // siren control 0.1, band control 0.1
        let handles_3 = InnerHandles::default();
        handles_3.siren_control.set_value(0.1);
        handles_3.band_control.set_value(0.1);
        let node_3 = create_node(&config, handles_3, &values);

        chart_config.output_title("siren control 0.1, band control 0.1");

        // siren control 0.5
        let handles_4 = InnerHandles::default();
        handles_4.siren_control.set_value(0.5);
        let node_4 = create_node(&config, handles_4, &values);

        chart_config.output_title("siren control 0.5");

        // siren control 0.5, band control 0.5
        let handles_5 = InnerHandles::default();
        handles_5.siren_control.set_value(0.5);
        handles_5.band_control.set_value(0.5);
        let node_5 = create_node(&config, handles_5, &values);

        chart_config.output_title("siren control 0.5, band control 0.5");

        // siren control 1.0
        let handles_6 = InnerHandles::default();
        handles_6.siren_control.set_value(1.0);
        let node_6 = create_node(&config, handles_6, &values);

        chart_config.output_title("siren control 1.0");

        // siren control 1.0, band control 1.0
        let handles_7 = InnerHandles::default();
        handles_7.siren_control.set_value(1.0);
        handles_7.band_control.set_value(1.0);
        let node_7 = create_node(&config, handles_7, &values);

        chart_config.output_title("siren control 1.0, band control 1.0");

        let test_stack = node_1 | node_2 | node_3 | node_4 | node_5 | node_6 | node_7;

        let chart_config = chart_config.build().unwrap();

        assert_audio_unit_snapshot!(test_stack, chart_config);
    }

    #[test]
    fn test_group() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let config = GroupConfig {
            channel: common::instrument::GroupChannel::Left,
            nodes: vec![
                // silent
                NodeConfig {
                    key: NodeKey(0, 0),
                    base_frequency: 70.0,
                    phase: 0.0,
                    divisions: 4,
                },
                // 0.1
                NodeConfig {
                    key: NodeKey(0, 1),
                    base_frequency: 140.0,
                    phase: 0.1,
                    divisions: 4,
                },
                // 0.5
                NodeConfig {
                    key: NodeKey(0, 2),
                    base_frequency: 280.0,
                    phase: 0.2,
                    divisions: 4,
                },
                // 1.0
                NodeConfig {
                    key: NodeKey(0, 3),
                    base_frequency: 560.0,
                    phase: 0.3,
                    divisions: 4,
                },
            ],
        };

        let group_handles = HashMap::from_iter((0..=3).map(|i| {
            let handles = InnerHandles::default();
            match i {
                0 => handles.siren_control.set_value(0.0),
                1 => handles.siren_control.set_value(0.1),
                2 => handles.siren_control.set_value(0.5),
                3 => handles.siren_control.set_value(1.0),
                _ => unreachable!(),
            }
            (NodeKey(0, i), handles)
        }));

        let node = create_group_node::<U4>(&config, group_handles, &values);

        let config = SnapshotConfigBuilder::default()
            .num_samples(4000)
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(node, config);
    }
}
