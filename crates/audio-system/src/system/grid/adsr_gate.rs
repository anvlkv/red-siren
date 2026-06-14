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
                let n = P::USIZE;
                let mut shape: Frame<f32, Prod<P, U2>> = Frame::default();
                let weight_sum = (n * (n + 1)) as f64 / 2.0;
                let mut remaining_duration = duration_s;

                for i in 0..n {
                    let is_last = i == n - 1;

                    // Increasing weights
                    let weight = ((i + 1) as f64).powf(2.0);
                    let mut d_step = duration_s * (weight / weight_sum);

                    // Force exact total duration despite floating-point error.
                    if is_last {
                        d_step = remaining_duration;
                    }
                    remaining_duration -= d_step;

                    let env_value = if is_last { 0.0 } else { 1.0 / (i + 1) as f64 };

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

pub fn create_adsr_gate<P: Size<f32> + Unsigned + ArrayLength>() -> An<AdsrGate<P>>
where
    P: Mul<U2>,
    Prod<P, U2>: Size<f32>,
    U1: Add<Prod<P, U2>>,
    Sum<U1, Prod<P, U2>>: Size<f32>,
{
    An(AdsrGate(PhantomData))
}
