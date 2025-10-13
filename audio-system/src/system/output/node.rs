use std::{cell::RefCell, mem};

use common::instrument::{GroupConfig, NodeConfig};
use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, B1},
};

use super::formant::*;
use super::siren::*;
use super::InnerHandles;

pub type S = f32;

pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<
                    // Binop<FrameMul<UInt<UTerm, B1>>,
                    Pipe<Constant<UInt<UTerm, B1>>, Sine<S>>,
                    // , Siren>,
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
            Join<UInt<UInt<UTerm, B1>, B1>>,
        >,
        FixedSvf<S, HighpassMode<S>>,
    >,
    SnoopBackend,
>;

pub const ACTIVATION_SNOOP_CAPACITY: usize = 16;
pub const OUTPUT_SNOOP_CAPACITY: usize = 256;

fn create_node(config: &NodeConfig, handles: InnerHandles) -> An<NodeType> {
    let InnerHandles {
        activation_snoop: _activation_snoop,
        output_snoop: output_snoop_backend,
        siren_control,
        band_control,
        ..
    } = handles;

    // Source
    (sine_hz::<S>(config.base_frequency as S))// * siren(siren_control))
        // Create resonator formants
        >> split::<U3>()
        >> ((formant::<1>(band_control.clone(), config.base_frequency as f32) * 1.0)
        | (formant::<2>(band_control.clone(), config.base_frequency as f32) * 0.8)
        | (formant::<3>(band_control.clone(), config.base_frequency as f32) * 0.6))
        >> join::<U3>()
        // High-pass filter to remove low-frequency rumble
        >> highpass_hz(80.0, 1.0)
        // Visualize
        >> output_snoop_backend
}

pub fn create_group_node<K>(
    config: &GroupConfig,
    group_handles: Vec<InnerHandles>,
) -> An<MultiBus<K, NodeType>>
where
    K: Size<f32> + Size<NodeType>,
{
    let nodes = config.nodes.clone();
    let handles_cell = RefCell::new(group_handles);
    busi::<K, _, _>(move |i| {
        let handle = mem::take(&mut handles_cell.borrow_mut()[i as usize]);
        create_node(&nodes[i as usize], handle)
    })
}
