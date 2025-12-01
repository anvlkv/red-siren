use std::sync::Arc;

#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::thingbuf::ThingBuf;

use crate::util::hash_str;

const THROW_ID: u64 = hash_str(concat!(module_path!(), "::ThrowCatchThrow"));
const CATCH_ID: u64 = hash_str(concat!(module_path!(), "::ThrowCatchCatch"));

#[derive(Clone)]
pub struct ThrowCatchThrow {
    sx: Arc<ThingBuf<f32>>,
}

pub fn throw<O>() -> (Arc<ThingBuf<f32>>, An<ThrowCatchThrow>)
where
    O: typenum::Unsigned + Size<f32>,
{
    let buffer = Arc::new(ThingBuf::new(O::USIZE));
    (buffer.clone(), An(ThrowCatchThrow { sx: buffer }))
}

impl AudioNode for ThrowCatchThrow {
    const ID: u64 = THROW_ID;

    type Inputs = U1;

    type Outputs = U0;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        _ = self.sx.push(input[0]);
        Frame::default()
    }
}

#[derive(Clone)]
pub struct ThrowCatchCatch<O>
where
    O: typenum::Unsigned + Size<f32>,
{
    rx: Arc<ThingBuf<f32>>,
    _o: std::marker::PhantomData<O>,
}

pub fn catch<O>(buffer: Arc<ThingBuf<f32>>) -> An<ThrowCatchCatch<O>>
where
    O: typenum::Unsigned + Size<f32>,
{
    An(ThrowCatchCatch {
        rx: buffer,
        _o: std::marker::PhantomData,
    })
}

impl<O> AudioNode for ThrowCatchCatch<O>
where
    O: typenum::Unsigned + Size<f32>,
{
    const ID: u64 = CATCH_ID;

    type Inputs = U0;

    type Outputs = O;

    fn tick(&mut self, _: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let mut frame = Frame::splat(0.0);
        for f in frame.iter_mut() {
            if let Some(sample) = self.rx.pop() {
                *f = sample;
            }
        }
        frame
    }
}

pub fn throw_catch<O>() -> (An<ThrowCatchThrow>, An<ThrowCatchCatch<O>>)
where
    O: typenum::Unsigned + Size<f32>,
{
    let (buffer, throw_node) = throw::<O>();
    let catch_node = catch::<O>(buffer);
    (throw_node, catch_node)
}
