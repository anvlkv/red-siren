use fundsp::hacker32::prelude::*;

use crate::instrument::util::hash_str;

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));
const SIREN_BASE_HZ: f32 = 0.5;

#[derive(Clone)]
pub struct Siren {
    a_var: Var,
    freq: f32,
    phase: f32,
    time: f32,
    prev_a: f32,
    sample_duration: f32,
}
//  let phi: f32 = t - a * sin(2.0 * t);
// sin(phi * SIREN_BASE_HZ * std::f32::consts::TAU)
impl AudioNode for Siren {
    const ID: u64 = SIREN_ID;

    type Inputs = U0;

    type Outputs = U1;

    fn tick(&mut self, _input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        // Get current modulation
        let a = self.a_var.value();

        // Advance accumulated phase by integrating dphi/dt = 1 - a'(t) sin(2t) - 2a cos(2t)
        let t = self.time;
        let dt = self.sample_duration;
        let a_prime = if dt > 0.0 {
            (a - self.prev_a) / dt
        } else {
            0.0
        };
        let dphi = (1.0 - a_prime * sin(2.0 * t) - 2.0 * a * cos(2.0 * t)) * dt;
        self.phase += dphi;
        self.time += dt;
        self.prev_a = a;

        // Output using accumulated phase
        let output = sin(self.phase * self.freq * std::f32::consts::TAU);

        [output].into()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.time = 0.0;
        self.prev_a = 0.0;
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_duration = 1.0 / sample_rate as f32;
    }
}

pub fn siren(a_var: Var) -> An<Siren> {
    let mut siren = Siren {
        a_var,
        freq: SIREN_BASE_HZ,
        phase: 0.0,
        time: 0.0,
        prev_a: 0.0,
        sample_duration: 1.0 / DEFAULT_SR as f32,
    };
    siren.reset();
    siren.set_sample_rate(DEFAULT_SR);
    An(siren)
}
