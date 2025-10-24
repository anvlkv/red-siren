use std::{cell::RefCell, collections::HashMap};

use common::{
    instrument::{GroupConfig, NodeConfig},
    NodeKey,
};
use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, B0, B1},
};

use super::formant::*;
use super::siren::*;
use super::InnerHandles;

use crate::util::S;

pub type OscType = Pipe<
    Pipe<
        Pipe<Constant<UInt<UTerm, B1>>, Split<UInt<UInt<UTerm, B1>, B0>>>,
        Stack<Stack<Sine<f32>, WaveSynth<UInt<UTerm, B1>>>, Var>,
    >,
    super::crossfade::EqualPowerCrossfade,
>;

pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<
                    Pipe<
                        Binop<
                            FrameMul<UInt<UTerm, B1>>,
                            OscType,
                            Pipe<Pipe<Pipe<Var, Follow<S>>, SnoopBackend>, Siren<S>>,
                        >,
                        Split<UInt<UInt<UTerm, B1>, B1>>,
                    >,
                    Stack<
                        Stack<
                            Unop<Formant<1>, FrameMulScalar<UInt<UTerm, B1>>>,
                            Unop<Formant<2>, FrameMulScalar<UInt<UTerm, B1>>>,
                        >,
                        Unop<Formant<3>, FrameMulScalar<UInt<UTerm, B1>>>,
                    >,
                >,
                Unop<Join<UInt<UInt<UTerm, B1>, B1>>, FrameMulScalar<UInt<UTerm, B1>>>,
            >,
            super::chorus::Chorus,
        >,
        FixedSvf<S, BellMode<S>>,
    >,
    SnoopBackend,
>;

pub const ACTIVATION_SNOOP_CAPACITY: usize = 8;
pub const OUTPUT_SNOOP_CAPACITY: usize = 1024;
const FOLLOW_RESPONSE_TIME_S: f32 = (1.0 / 75.0) * 25.0;
const NODE_BELL_Q: f32 = 0.085;
const NODE_BELL_GAIN_DB: f32 = 1.0 / 3.0;

fn create_node(config: &NodeConfig, handles: InnerHandles) -> An<NodeType> {
    let InnerHandles {
        activation_snoop,
        output_snoop,
        siren_control,
        band_control,
        ..
    } = handles;

    let source = constant(config.base_frequency as S)
        >> split::<U2>()
        >> (sine_phase::<S>(config.phase as S) | saw() | An(band_control.clone()))
        >> super::crossfade::equal_power_crossfade();

    let siren_activation =
        An(siren_control) >> follow(FOLLOW_RESPONSE_TIME_S) >> activation_snoop >> siren();
    let formants = (formant::<1>(band_control.clone(), config.base_frequency as S) * 1.0)
        | (formant::<2>(band_control.clone(), config.base_frequency as S) * 0.8)
        | (formant::<3>(band_control.clone(), config.base_frequency as S) * 0.6);

    (source * siren_activation)
        >> split::<U3>()
        >> formants
        >> (join::<U3>() * (1.0 / (1.0 + 0.8 + 0.6)))
        >> super::chorus::chorus(config.key.idx() as u64, 0.05, 0.75, 0.75)
        >> bell_hz(config.base_frequency as S, NODE_BELL_Q, NODE_BELL_GAIN_DB)
        >> output_snoop
}

pub type GroupType<K> = MultiBus<K, NodeType>;

pub fn create_group_node<K>(
    config: &GroupConfig,
    group_handles: HashMap<NodeKey, InnerHandles>,
) -> An<GroupType<K>>
where
    K: Size<f32> + Size<NodeType>,
{
    let nodes = config.nodes.clone();
    let handles_cell = RefCell::new(group_handles);
    busi::<K, _, _>(move |i| {
        let key = nodes[i as usize].key;
        let mut handles = handles_cell.borrow_mut();
        let handle = handles.remove(&key).expect("missing handle for node key");
        create_node(&nodes[i as usize], handle)
    })
}
