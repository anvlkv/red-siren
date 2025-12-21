use fundsp::prelude::*;

use crate::util::{SComplex, S};

#[derive(Clone)]
pub struct ExcitementControl {
    pub primary: Shared,
    pub secondary: Shared,
}

#[allow(clippy::unnecessary_cast)]
impl ExcitementControl {
    pub fn new(primary: Shared, secondary: Shared) -> Self {
        Self { primary, secondary }
    }

    pub fn new_primary(primary: Shared) -> Self {
        Self::new(primary, shared(0.0))
    }

    pub fn value(&self) -> SComplex {
        SComplex::new(self.primary_value(), self.secondary_value())
    }

    pub fn primary_value(&self) -> S {
        self.primary.value() as S
    }

    pub fn secondary_value(&self) -> S {
        self.secondary.value() as S
    }

    pub fn set_value(&self, (re, im): (S, S)) {
        self.primary.set_value(re as f32);
        self.secondary.set_value(im as f32);
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
