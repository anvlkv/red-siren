use fundsp::prelude::*;
use num_complex::Complex;

#[derive(Clone)]
pub struct ExcitementControl {
    pub primary: Shared,
    pub secondary: Shared,
}

impl ExcitementControl {
    pub fn new(primary: Shared, secondary: Shared) -> Self {
        Self { primary, secondary }
    }

    pub fn new_primary(primary: Shared) -> Self {
        Self::new(primary, shared(0.0))
    }

    pub fn value<S: Real + Float>(&self) -> Complex<S> {
        Complex::new(self.primary_value(), self.secondary_value())
    }

    pub fn primary_value<S: Real + Float>(&self) -> S {
        S::from_f32(self.primary.value())
    }

    pub fn secondary_value<S: Real + Float>(&self) -> S {
        S::from_f32(self.secondary.value())
    }

    pub fn set_value<S: Real + Float>(&self, (re, im): (S, S)) {
        self.primary.set_value(re.to_f32());
        self.secondary.set_value(im.to_f32());
    }

    pub fn reset(&self) {
        self.primary.set_value(0.0);
        self.secondary.set_value(0.0);
    }
}

impl Default for ExcitementControl {
    fn default() -> Self {
        Self {
            primary: shared(0.0),
            secondary: shared(0.0),
        }
    }
}
