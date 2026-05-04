use common::NodeKey;
use fundsp::prelude::*;
use num_complex::Complex;

#[derive(Clone)]
pub struct Control {
    pub key: NodeKey,
    pub real: Shared,
    pub imaginary: Shared,
}

impl Control {
    pub fn value<S: Real + Float>(&self) -> Complex<S> {
        Complex::new(self.primary_value(), self.secondary_value())
    }

    pub fn primary_value<S: Real + Float>(&self) -> S {
        S::from_f32(self.real.value())
    }

    pub fn secondary_value<S: Real + Float>(&self) -> S {
        S::from_f32(self.imaginary.value())
    }

    pub fn set_value<S: Real + Float>(&self, (real, imaginary): (S, S)) {
        self.real.set_value(real.to_f32());
        self.imaginary.set_value(imaginary.to_f32());
    }

    pub fn reset(&self) {
        self.real.set_value(0.0);
        self.imaginary.set_value(0.0);
    }

    pub fn control_node(&self) -> An<ControlNode> {
        An(ControlNode {
            _key: self.key,
            real: Var::new(&self.real),
            imaginary: Var::new(&self.imaginary),
        })
    }
}

const CONTROL_NODE_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::ControlNode"));

#[derive(Clone)]
pub struct ControlNode {
    _key: NodeKey,
    real: Var,
    imaginary: Var,
}

impl AudioNode for ControlNode {
    const ID: u64 = CONTROL_NODE_ID;
    type Inputs = U0;
    type Outputs = U2;

    fn tick(&mut self, _input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let real = self.real.value();
        let imaginary = self.imaginary.value();
        [real, imaginary].into()
    }

    fn reset(&mut self) {
        self.real.set_value(0.0);
        self.imaginary.set_value(0.0);
    }
}
