use std::{cell::RefCell, collections::HashMap};

use common::{
    instrument::{GroupConfig, NodeConfig},
    NodeKey,
};
use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, B1},
};

use super::formant::*;
use super::siren::*;
use super::InnerHandles;

use crate::util::S;

pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<
                    Binop<
                        FrameMul<UInt<UTerm, B1>>,
                        Pipe<Constant<UInt<UTerm, B1>>, Sine<S>>,
                        Pipe<Pipe<Pipe<Var, Follow<S>>, SnoopBackend>, Siren>,
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
        FixedSvf<S, BellMode<S>>,
    >,
    SnoopBackend,
>;

pub const ACTIVATION_SNOOP_CAPACITY: usize = 16;
pub const OUTPUT_SNOOP_CAPACITY: usize = 256;
const FOLLOW_RESPONSE_TIME_S: f32 = 1.0 / 75.0;
const NODE_BELL_Q: f32 = 0.3142;
const NODE_BELL_GAIN_DB: f32 = 1.7;

fn create_node(config: &NodeConfig, handles: InnerHandles) -> An<NodeType> {
    let InnerHandles {
        activation_snoop,
        output_snoop,
        siren_control,
        band_control,
        ..
    } = handles;

    let source = sine_hz::<S>(config.base_frequency as S);
    let siren_activation =
        An(siren_control) >> follow(FOLLOW_RESPONSE_TIME_S) >> activation_snoop >> siren();
    let formants = (formant::<1>(band_control.clone(), config.base_frequency as f32) * 1.0)
        | (formant::<2>(band_control.clone(), config.base_frequency as f32) * 0.8)
        | (formant::<3>(band_control.clone(), config.base_frequency as f32) * 0.6);

    // Source
    (source * siren_activation)
        // Create resonator formants
        >> split::<U3>()
        >> formants
        >> (join::<U3>() * 0.104167)
        // Node bell filter
        >> bell_hz(config.base_frequency as S, NODE_BELL_Q, NODE_BELL_GAIN_DB)
        // Visualize
        >> output_snoop
}

pub type GroupType<K> = Pipe<MultiBus<K, NodeType>, FixedSvf<S, HighpassMode<S>>>;

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
    // High-pass filter to remove low-frequency rumble
    >> highpass_hz(80.0, 1.0)
}
