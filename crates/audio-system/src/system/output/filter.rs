use fundsp::hacker32::prelude::*;

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
            Stack<
                Binop<FrameMul<U1>, Pass, Pipe<Var, Follow<S>>>,
                Binop<FrameMul<U1>, Pass, Binop<FrameSub<U1>, Constant<U1>, Pipe<Var, Follow<S>>>>,
            >,
        >,
        Stack<Stack<Stack<Pass, Constant<U1>>, Constant<U1>>, Pass>,
    >,
    Binop<FrameAdd<U1>, FbBiquad<S, ResonatorBiquad<S>, SoftCrush>, Pass>,
>;

const RESONATOR_Q: f32 = std::f32::consts::PI / 10.0;
const SWITCH_FOLLOW_RESPONSE_S: f32 = (1.0 / 75.0) * 3.0;
const SHAPE_VALUE: f32 = 0.75;

pub fn create_filter(FilterHandles { control, freq }: FilterHandles) -> An<FilterType> {
    let control = An(control) >> follow(SWITCH_FOLLOW_RESPONSE_S);

    split::<U2>()
        >> ((pass() * control.clone()) | (pass() * (constant(1.0) - control)))
        >> ((pass() | constant(freq as S) | constant(RESONATOR_Q)) | pass())
        >> (fresonator(SoftCrush(SHAPE_VALUE)) + pass())
}
