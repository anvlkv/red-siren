use fundsp::prelude::*;

#[derive(Clone)]
pub struct Pattern {
    pub data: Vec<Complex32>,
    pub tolerance_re: f32,
    pub tolerance_im: f32,
}

impl Pattern {
    pub fn new(data: Vec<Complex32>, tolerance_re: f32, tolerance_im: f32) -> Self {
        Self {
            data,
            tolerance_re,
            tolerance_im,
        }
    }

    pub fn matches(&self, input: &[Complex32]) -> bool {
        if self.data.len() != input.len() {
            return false;
        }

        for (a, b) in self.data.iter().zip(input.iter()) {
            if (a.re - b.re).abs() > self.tolerance_re || (a.im - b.im).abs() > self.tolerance_im {
                return false;
            }
        }

        true
    }
}
