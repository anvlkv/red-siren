use common::instrument::NodeConfig;
use fundsp::{
    hacker::prelude::*,
    typenum::{UInt, UTerm, B1},
};

use crate::{system::values::FineTunedValue, util::S, values::FineTunedValues};

#[derive(Clone)]
pub struct FilterHandles {
    pub control_a_b: Var,
    pub control: Var,
    pub config: NodeConfig,
}

type Control = Pipe<Var, Follow<S>>;

type PowCents = Pipe<Stack<Constant<UInt<UTerm, B1>>, Control>, super::pow::Pow<S>>;

type AllPassChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
        FineTunedValue,
    >,
    Svf<S, AllpassMode<S>>,
>;

type MoogChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
        FineTunedValue,
    >,
    Moog<S, UInt<UInt<UTerm, B1>, B1>>,
>;

type ShelfChain = Pipe<
    Stack<
        Stack<
            Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
            FineTunedValue,
        >,
        FineTunedValue,
    >,
    Svf<S, HighshelfMode<S>>,
>;

type PassChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
        FineTunedValue,
    >,
    Svf<S, LowpassMode<S>>,
>;

type CrossFadeChain =
    Pipe<Stack<Stack<Pass, Pass>, Control>, super::crossfade::EqualPowerCrossfade>;

pub type FilterType = Pipe<
    Pipe<Split<U2>, Stack<Pipe<ShelfChain, PassChain>, Pipe<AllPassChain, MoogChain>>>,
    CrossFadeChain,
>;

pub fn create_filter(handles: FilterHandles, finetuned_values: &FineTunedValues) -> An<FilterType> {
    let FilterHandles {
        control_a_b,
        control,
        config,
    } = handles;

    let FineTunedValues {
        filter_switch_follow_response_s,
        filter_allpass_q,
        filter_moog_q,
        filter_shelf_q,
        filter_shelf_gain,
        filter_pass_q,
        ..
    } = finetuned_values.clone();

    // Get the follow response time value
    #[cfg(feature = "editor")]
    let follow_time = filter_switch_follow_response_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = filter_switch_follow_response_s.value()[0];

    let control_a_b: An<Control> = An(control_a_b) >> follow::<S>(follow_time as S);
    let control: An<Control> = An(control) >> follow::<S>(follow_time as S);

    let pow_cents: An<PowCents> =
        (constant(config.cents as f32) | control.clone()) >> super::pow::pow::<S>();

    // B chains
    let filter_allpass_chain: An<AllPassChain> =
        (pass() | (constant(config.formant_hz(4) as f32) * pow_cents.clone()) | filter_allpass_q)
            >> allpass::<S>();

    let filter_moog_chain: An<MoogChain> =
        (pass() | (constant(config.formant_hz(5) as f32) * pow_cents.clone()) | filter_moog_q)
            >> moog::<S>();

    // A chains
    let filter_shelf_chain: An<ShelfChain> = (pass()
        | (constant(config.formant_hz(4) as f32) * pow_cents.clone())
        | filter_shelf_q
        | filter_shelf_gain)
        >> highshelf::<S>();

    let filter_pass_chain: An<PassChain> =
        (pass() | (constant(config.formant_hz(5) as f32) * pow_cents) | filter_pass_q)
            >> lowpass::<S>();

    let cross_fade_chain: An<CrossFadeChain> =
        (pass() | pass() | control_a_b) >> super::crossfade::equal_power_crossfade();

    split::<U2>()
        >> ((filter_shelf_chain >> filter_pass_chain) | (filter_allpass_chain >> filter_moog_chain))
        >> cross_fade_chain
}
