use fundsp::{
    hacker::prelude::*,
    typenum::{UInt, UTerm, B0, B1},
};

use crate::util::S;

#[derive(Clone)]
pub struct FilterHandles {
    pub control: Var,
    pub freq: f64,
}

pub type FilterType = Pipe<
    Pipe<
        Pipe<
            Split<UInt<UInt<UTerm, B1>, B0>>,
            Stack<Stack<Stack<Pass, Pass>, Constant<UInt<UTerm, B1>>>, Constant<UInt<UTerm, B1>>>,
        >,
        Stack<
            Stack<Pass, Pipe<Svf<S, AllpassMode<S>>, Moog<S, UInt<UTerm, B1>>>>,
            Pipe<Var, Follow<S>>,
        >,
    >,
    super::crossfade::EqualPowerCrossfade,
>;

const FILTER_Q: S = 0.6;
const SWITCH_FOLLOW_RESPONSE_S: S = (1.0 / 75.0) * 3.0;

pub fn create_filter(FilterHandles { control, freq }: FilterHandles) -> An<FilterType> {
    let control = An(control) >> follow(SWITCH_FOLLOW_RESPONSE_S);

    split::<U2>()
        >> (pass() | pass() | constant(freq as f32) | constant(FILTER_Q as f32))
        >> (pass() | (allpass() >> moog_hz((freq * 0.7) as S, FILTER_Q)) | control)
        >> super::crossfade::equal_power_crossfade()
}
