use std::marker::PhantomData;
use std::ops::{Add, Mul};

use fundsp::numeric_array::ArrayLength;
use fundsp::prelude::*;
use fundsp::typenum::{Prod, Sum, Unsigned};

use crate::system::adsr_3d::Adsr3D;

use super::scheduler::SchedulingEvent;
use super::FrameEncodedSignal;

#[derive(Clone)]
pub struct AdsrGate<P: Size<f32> + Unsigned + ArrayLength>(PhantomData<P>)
where
    P: Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>;

impl<P: Size<f32> + Unsigned + ArrayLength> AudioNode for AdsrGate<P>
where
    P: Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>,
{
    const ID: u64 = crate::util::hash_str(concat!(module_path!(), "::AdsrGate"));

    type Inputs = <SchedulingEvent as FrameEncodedSignal>::Size;

    type Outputs = <Adsr3D<f32, P> as AudioNode>::Inputs;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let mut output = Frame::default();
        let event = SchedulingEvent::decode(input);
        match event {
            SchedulingEvent::Event { value, duration_s } => {
                let mut shape: Frame<f32, Prod<P, U2>> = Frame::default();
                let d_even_step = duration_s / (P::USIZE as f64);
                let mut remaining_duration = duration_s;
                for i in 0..P::USIZE {
                    let i_rev = P::USIZE - i;
                    let d_step = if i == P::USIZE - 1 {
                        remaining_duration
                    } else {
                        d_even_step / i_rev as f64 + remaining_duration / i_rev as f64
                    };
                    remaining_duration -= d_step;
                    let env_value = i_rev as f64 / (P::USIZE as f64);
                    shape[i * 2] = d_step as f32;
                    shape[i * 2 + 1] = env_value as f32;
                }
                output[0] = value as f32;
                output[1..].copy_from_slice(&shape);
            }
            SchedulingEvent::None => {}
        }

        output
    }
}
