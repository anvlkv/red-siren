use std::{cell::RefCell, collections::HashMap};

use common::{instrument::GroupConfig, NodeKey};
use fundsp::prelude::*;

use crate::{
    output::{abs::Abs, div::Div, InnerHandles},
    util::S,
};

#[allow(clippy::excessive_precision)]
const METRO_FREQ_REDUCTION: S = 750.750750;

type ControlledFreq = Binop<FrameMul<U1>, Constant<U1>, Binop<FrameSub<U1>, Constant<U1>, Var>>;

type MetroMod = Pipe<
    Pipe<
        Binop<FrameMul<U1>, Pipe<Stack<ControlledFreq, Constant<U1>>, Div<S>>, Constant<U1>>,
        Binop<FrameSub<U1>, WaveSynth<U1>, Constant<U1>>,
    >,
    Abs,
>;

type PanControl =
    Pipe<Binop<FrameSub<U1>, Constant<U1>, Binop<FrameMul<U1>, Var, Constant<U1>>>, Shaper<ClipTo>>;

pub type MetroType = Pipe<
    Pipe<Stack<Pass, PanControl>, Panner<U2>>,
    Binop<FrameAdd<U1>, Binop<FrameMul<U1>, Pass, MetroMod>, Pass>,
>;

#[allow(clippy::unnecessary_cast)]
fn create_metro(
    key_control: &Shared,
    band_control: &Shared,
    node_freq: S,
    divisions: u32,
    index: u32,
) -> An<MetroType> {
    let controlled_freq: An<ControlledFreq> =
        constant((node_freq / METRO_FREQ_REDUCTION) as f32) * (constant(1.1) - var(band_control));

    let metro_mod: An<MetroMod> = (((controlled_freq | constant(divisions as f32))
        >> super::div::div::<S>())
        * constant((index + 1) as f32))
        >> (square() - constant(1.0))
        >> super::abs::abs();

    let pan_control: An<PanControl> =
        (constant(1.0) - var(key_control) * constant(2.0)) >> clip_to(-0.7, 1.0);

    (pass() | pan_control) >> panner() >> ((pass() * metro_mod) + pass())
}

pub type MetroBusType<K> = Binop<FrameMul<U1>, MultiBus<K, MetroType>, Constant<U1>>;

pub fn metro_busi<K>(
    config: &GroupConfig,
    group_handles: &HashMap<NodeKey, InnerHandles>,
) -> An<MetroBusType<K>>
where
    K: Size<S> + Size<MetroType>,
{
    let nodes = config.nodes.clone();
    let controls = RefCell::new(group_handles.clone());

    let buss: An<MultiBus<K, MetroType>> = busi::<K, _, _>(move |i| {
        let i = i as usize;
        let mut handles = controls.borrow_mut();
        let base_freq = nodes[i].frequency;
        let InnerHandles {
            band_control,
            key_control,
            ..
        } = handles.remove(&nodes[i].key).unwrap();

        create_metro(
            &key_control,
            &band_control,
            base_freq as S,
            nodes[i].divisions,
            i as u32,
        )
    });

    buss * constant(1.0 / K::USIZE as f32)
}
