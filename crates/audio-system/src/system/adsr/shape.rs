use fundsp::{Float, Real};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Shape<S: Real + Float> {
    pub running_value: S,
    pub control_a: S,
    pub control_b: S,
}

impl<S: Real + Float> Shape<S> {
    pub fn zero() -> Self {
        Self {
            running_value: S::zero(),
            control_a: S::zero(),
            control_b: S::zero(),
        }
    }
}
