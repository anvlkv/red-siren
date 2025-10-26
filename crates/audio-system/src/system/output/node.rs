use std::{cell::RefCell, collections::HashMap};

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

// Type alias for the siren activation with follow envelope
type SirenActivation = Pipe<Pipe<Var, Follow<S>>, SnoopBackend>;

// Type alias for the siren with all 5 inputs stacked
type SirenWithInputs = Pipe<
    Stack<
        Stack<Stack<Stack<SirenActivation, FineTunedValue>, FineTunedValue>, FineTunedValue>,
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
type FormantFilter<const N: u8> = Pipe<Stack<Pass, FineTunedValue>, super::formant::Formant<N>>;

// Type alias for all three formant filters stacked with gain scaling
type FormantBank = Stack<
    Stack<
        Unop<FormantFilter<1>, FrameMulScalar<UInt<UTerm, B1>>>,
        Unop<FormantFilter<2>, FrameMulScalar<UInt<UTerm, B1>>>,
    >,
    Unop<FormantFilter<3>, FrameMulScalar<UInt<UTerm, B1>>>,
>;

// Type alias for the bell filter with its 4 inputs
type BellFilter =
    Pipe<Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, Var>, Var>, Svf<S, BellMode<S>>>;

// Complete node type composed from the smaller parts
pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<
                    Pipe<
                        Pipe<
                            Pipe<SirenWithInputs, Split<UInt<UInt<UTerm, B1>, B0>>>,
                            Binop<FrameMul<UInt<UTerm, B1>>, SourceOscillator, Pass>,
                        >,
                        Split<UInt<UInt<UTerm, B1>, B1>>,
                    >,
                    FormantBank,
                >,
                Join<UInt<UInt<UTerm, B1>, B1>>,
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
        activation_snoop,
        output_snoop,
        siren_control,
        band_control,
        ..
    } = handles;

    let source: An<SourceOscillator> = ((super::abs::abs() >> clip_to(0.75, 1.0))
        * constant(config.base_frequency as f32))
        >> split::<U2>()
        >> (sine_phase::<S>(config.phase as f32) | saw() | An(band_control.clone()))
        >> super::crossfade::equal_power_crossfade();

    // Get the follow response time value
    let follow_time_node = values.node_follow_response_time_s.clone();
    let follow_time = follow_time_node.value();

    let siren_activation: An<SirenActivation> =
        An(siren_control) >> follow::<S>(follow_time as S) >> activation_snoop;

    // Stack inputs for siren (5 inputs total: excitement + 4 fine-tuned values)
    let siren_output: An<SirenWithInputs> = (siren_activation
        | values.siren_base_hz.clone()
        | values.siren_max_frequency_hz.clone()
        | values.siren_excitement_pause_limit.clone()
        | values.siren_base_pause_duration.clone())
        >> siren::<S>();

    // Formants with base_q as input
    let formant1: An<FormantFilter<1>> = (pass() | values.formant_base_q.clone())
        >> formant::<1>(band_control.clone(), config.base_frequency as S);
    let formant2: An<FormantFilter<2>> = (pass() | values.formant_base_q.clone())
        >> formant::<2>(band_control.clone(), config.base_frequency as S);
    let formant3: An<FormantFilter<3>> = (pass() | values.formant_base_q.clone())
        >> formant::<3>(band_control.clone(), config.base_frequency as S);

    let formants: An<FormantBank> = (formant1 * 1.0) | (formant2 * 0.8) | (formant3 * 0.6);

    let bell_filter: An<BellFilter> = (pass()
        | constant(config.base_frequency as f32)
        | values.node_bell_q.clone()
        | values.node_bell_gain_db.clone())
        >> bell();

    siren_output
        >> split::<U2>()
        >> (source * pass())
        >> split::<U3>()
        >> formants
        >> join::<U3>()
        >> super::chorus::chorus(config.key.idx() as u64, 0.05, 0.75, 0.75)
        >> bell_filter
        >> output_snoop
}

pub type GroupType<K> = MultiBus<K, NodeType>;

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
    busi::<K, _, _>(move |i| {
        let key = nodes[i as usize].key;
        let mut handles = handles_cell.borrow_mut();
        let handle = handles.remove(&key).expect("missing handle for node key");
        create_node(&nodes[i as usize], handle, &values_clone)
    })
}
