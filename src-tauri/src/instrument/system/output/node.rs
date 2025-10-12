use std::{cell::RefCell, mem};

use common::instrument::{GroupConfig, NodeConfig};
use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, B1},
};

use super::siren::*;
use super::InnerHandles;

pub type S = f32;

pub type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Pipe<
                    Binop<
                        FrameMul<UInt<UTerm, B1>>,
                        Pipe<Constant<UInt<UTerm, B1>>, Sine<S>>,
                        Siren,
                    >,
                    Unop<Resonator<S, UInt<UTerm, B1>>, FrameMulScalar<UInt<UTerm, B1>>>,
                >,
                Unop<Resonator<S, UInt<UTerm, B1>>, FrameMulScalar<UInt<UTerm, B1>>>,
            >,
            Unop<Resonator<S, UInt<UTerm, B1>>, FrameMulScalar<UInt<UTerm, B1>>>,
        >,
        FixedSvf<S, HighpassMode<S>>,
    >,
    SnoopBackend,
>;

pub const ACTIVATION_SNOOP_CAPACITY: usize = 16;
pub const OUTPUT_SNOOP_CAPACITY: usize = 256;

fn create_node(config: &NodeConfig, handles: InnerHandles, _a_coef: f32) -> An<NodeType> {
    let InnerHandles {
        activation_snoop: _activation_snoop,
        output_snoop: output_snoop_backend,
        siren_control,
        band_control: _band_control,
        ..
    } = handles;

    // Formant parameters
    let f1 = 730.0;
    let f2 = 1090.0;
    let f3 = 2440.0;

    // Bandwidths (in Hz) for more natural sound
    let bw1 = 100.0;
    let bw2 = 120.0;
    let bw3 = 150.0;

    // Source
    (sine_hz::<S>(config.base_frequency as S) * siren(siren_control))
        // Create resonator formants
        >> (resonator_hz(f1, bw1) * 1.0)
        >> (resonator_hz(f2, bw2) * 0.8)
        >> (resonator_hz(f3, bw3) * 0.6)
        // High-pass filter to remove low-frequency rumble
        >> highpass_hz(80.0, 1.0) >> output_snoop_backend
}

pub fn create_group_node<K>(
    config: &GroupConfig,
    group_handles: Vec<InnerHandles>,
) -> An<MultiBus<K, NodeType>>
where
    K: Size<f32> + Size<NodeType>,
{
    let nodes = config.nodes.clone();
    let a_coef = config.a_coef;
    let handles_cell = RefCell::new(group_handles);
    busi::<K, _, _>(move |i| {
        let handle = mem::take(&mut handles_cell.borrow_mut()[i as usize]);
        create_node(&nodes[i as usize], handle, a_coef)
    })
}
