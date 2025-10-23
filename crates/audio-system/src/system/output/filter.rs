use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, B1},
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
            Split<U2>,
            Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, Constant<UInt<UTerm, B1>>>, Pass>,
        >,
        Stack<Stack<Svf<S, AllpassMode<S>>, Pass>, Pipe<Var, Follow<f32>>>,
    >,
    super::crossfade::EqualPowerCrossfade,
>;

const FILTER_Q: f32 = 0.3;
const SWITCH_FOLLOW_RESPONSE_S: f32 = (1.0 / 75.0) * 3.0;

pub fn create_filter(FilterHandles { control, freq }: FilterHandles) -> An<FilterType> {
    let control = An(control) >> follow(SWITCH_FOLLOW_RESPONSE_S);

    split::<U2>()
        >> ((pass() | constant(freq as S) | constant(FILTER_Q)) | pass())
        >> (allpass() | pass() | control)
        >> super::crossfade::equal_power_crossfade()
}
