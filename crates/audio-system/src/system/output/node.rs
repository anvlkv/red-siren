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

// Type alias for the siren excitement with follow envelope
type SirenExcitement = Pipe<Pipe<Var, Follow<S>>, SnoopBackend>;

// Type alias for the siren with all 5 inputs stacked
type SirenWithInputs = Pipe<
    Stack<Stack<Stack<SirenExcitement, FineTunedValue>, FineTunedValue>, FineTunedValue>,
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
        Unop<FormantFilter<1>, FrameMulScalar<UInt<UTerm, B1>>>,
        Unop<FormantFilter<2>, FrameMulScalar<UInt<UTerm, B1>>>,
    >,
    Unop<FormantFilter<3>, FrameMulScalar<UInt<UTerm, B1>>>,
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
    let siren_output: An<SirenWithInputs> =
        (siren_excitement | siren_alpha | siren_beta | siren_gamma) >> siren::<S>();

    // Formants with base_q as input
    let formant1: An<FormantFilter<1>> =
        (pass() | An(band_control.clone()) | formant_base_q.clone())
            >> formant::<1>(config.base_frequency as S, config.divisions);
    let formant2: An<FormantFilter<2>> =
        (pass() | An(band_control.clone()) | formant_base_q.clone())
            >> formant::<2>(config.base_frequency as S, config.divisions);
    let formant3: An<FormantFilter<3>> = (pass() | An(band_control.clone()) | formant_base_q)
        >> formant::<3>(config.base_frequency as S, config.divisions);

    let formants: An<FormantBank> = (formant1 * 1.0) >> (formant2 * 0.8) >> (formant3 * 0.6);

    let bell_filter: An<BellFilter> =
        (pass() | constant(config.base_frequency as f32) | node_bell_q | node_bell_gain_db)
            >> bell();

    siren_output
        >> split::<U2>()
        >> (source * pass())
        >> formants
        >> super::chorus::chorus(config.key.idx() as u64, 0.15, 0.75, 0.75)
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
