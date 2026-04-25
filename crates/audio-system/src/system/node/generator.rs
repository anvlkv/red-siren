use std::marker::PhantomData;

use common::instrument::NodeConfig;
use fundsp::prelude::*;

const GENERATOR_ID: u64 = crate::util::hash_str("NodeGenerator");

#[derive(Clone)]
pub struct NodeGenerator<S: Float> {
    inner: Box<dyn AudioUnit>,
    _sample_type: PhantomData<S>,
}

impl<S: Float> NodeGenerator<S> {
    pub fn new(config: &NodeConfig) -> Self {
        let inner = Net::new(1, 1);

        Self {
            inner: Box::new(inner),
            _sample_type: PhantomData,
        }
    }
}

impl<S: Float> AudioNode for NodeGenerator<S> {
    const ID: u64 = GENERATOR_ID;

    type Inputs = U1;
    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let mut output = Frame::default();
        self.inner.tick(input, &mut output);

        output
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        self.inner.process(size, input, output);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner.set_sample_rate(sample_rate);
    }

    fn allocate(&mut self) {
        self.inner.allocate();
    }

    fn set_hash(&mut self, hash: u64) {
        self.inner.set_hash(hash);
        self.reset();
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}
