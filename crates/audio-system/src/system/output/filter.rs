use fundsp::{
    hacker::prelude::*,
    typenum::{UInt, UTerm, B1},
};

use crate::{system::values::FineTunedValue, util::S};

#[derive(Clone)]
pub struct FilterHandles {
    pub control: Var,
    pub freq: f64,
}

type AllPassChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, FineTunedValue>>,
        FineTunedValue,
    >,
    Svf<S, AllpassMode<S>>,
>;

type MoogChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, FineTunedValue>>,
        FineTunedValue,
    >,
    Moog<S, UInt<UInt<UTerm, B1>, B1>>,
>;

type CrossFadeChain =
    Pipe<Stack<Stack<Pass, Pass>, Pipe<Var, Follow<f32>>>, super::crossfade::EqualPowerCrossfade>;

pub type FilterType =
    Pipe<Pipe<Split<U2>, Stack<Pass, Pipe<AllPassChain, MoogChain>>>, CrossFadeChain>;

pub fn create_filter(
    handles: FilterHandles,
    filter_allpass_q: An<FineTunedValue>,
    filter_allpass_freq_ratio: An<FineTunedValue>,
    filter_moog_freq_ratio: An<FineTunedValue>,
    filter_moog_q: An<FineTunedValue>,
    filter_switch_follow_response_s: An<FineTunedValue>,
) -> An<FilterType> {
    let FilterHandles { control, freq } = handles;

    // Get the follow response time value
    #[cfg(feature = "editor")]
    let follow_time = filter_switch_follow_response_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = filter_switch_follow_response_s.value()[0];

    let control = An(control) >> follow(follow_time);

    let filter_allpass_chain: An<AllPassChain> =
        (pass() | (constant(freq as f32) * filter_allpass_freq_ratio) | filter_allpass_q)
            >> allpass::<S>();

    let filter_moog_chain: An<MoogChain> =
        (pass() | (constant(freq as f32) * filter_moog_freq_ratio) | filter_moog_q) >> moog::<S>();

    let cross_fade_chain: An<CrossFadeChain> =
        (pass() | pass() | control) >> super::crossfade::equal_power_crossfade();

    split::<U2>() >> (pass() | (filter_allpass_chain >> filter_moog_chain)) >> cross_fade_chain
}
