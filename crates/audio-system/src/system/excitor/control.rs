use fundsp::prelude::*;
use num_complex::Complex;

#[derive(Clone)]
pub struct Control {
    /// E.g. strength of the hit
    pub real: Shared,
    /// E.g. distance from the center of the drum
    pub imaginary: Shared,
}

impl Control {
    pub fn new(real: Shared, imaginary: Shared) -> Self {
        Self { real, imaginary }
    }

    pub fn new_real(real: Shared) -> Self {
        Self::new(real, shared(0.0))
    }

    pub fn value<S: Real + Float>(&self) -> Complex<S> {
        Complex::new(self.real_value(), self.imaginary_value())
    }

    pub fn real_value<S: Real + Float>(&self) -> S {
        S::from_f32(self.real.value())
    }

    pub fn imaginary_value<S: Real + Float>(&self) -> S {
        S::from_f32(self.imaginary.value())
    }

    pub fn primary_value<S: Real + Float>(&self) -> S {
        self.real_value()
    }
    pub fn secondary_value<S: Real + Float>(&self) -> S {
        self.imaginary_value()
    }
    pub fn new_primary(real: Shared) -> Self {
        Self::new(real, shared(0.0))
    }

    pub fn set_value<S: Real + Float>(&self, (re, im): (S, S)) {
        self.real.set_value(re.to_f32());
        self.imaginary.set_value(im.to_f32());
    }

    pub fn reset(&self) {
        self.real.set_value(0.0);
        self.imaginary.set_value(0.0);
    }
}

impl Default for Control {
    fn default() -> Self {
        Self {
            real: shared(0.0),
            imaginary: shared(0.0),
        }
    }
}
