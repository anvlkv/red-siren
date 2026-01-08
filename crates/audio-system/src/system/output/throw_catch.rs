use std::sync::Arc;

use fundsp::prelude::*;
use fundsp::thingbuf::ThingBuf;

use crate::util::hash_str;

const THROW_ID: u64 = hash_str(concat!(module_path!(), "::ThrowCatchThrow"));
const CATCH_ID: u64 = hash_str(concat!(module_path!(), "::ThrowCatchCatch"));

#[derive(Clone)]
pub struct ThrowCatchThrow {
    sx: Arc<ThingBuf<f32>>,
}

pub fn throw(size: usize) -> (Arc<ThingBuf<f32>>, An<ThrowCatchThrow>) {
    let buffer = Arc::new(ThingBuf::new(size));
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
pub struct ThrowCatchCatch {
    rx: Arc<ThingBuf<f32>>,
}

pub fn catch(buffer: Arc<ThingBuf<f32>>) -> An<ThrowCatchCatch> {
    An(ThrowCatchCatch { rx: buffer })
}

impl AudioNode for ThrowCatchCatch {
    const ID: u64 = CATCH_ID;

    type Inputs = U0;

    type Outputs = U1;

    fn tick(&mut self, _: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        [self.rx.pop().unwrap_or_default()].into()
    }
}

pub fn throw_catch(size: usize) -> (An<ThrowCatchThrow>, An<ThrowCatchCatch>) {
    let (buffer, throw_node) = throw(size);
    let catch_node = catch(buffer);
    (throw_node, catch_node)
}
