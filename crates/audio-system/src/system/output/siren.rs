use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));
const SIREN_BASE_HZ: f32 = 0.5;

#[derive(Clone)]
pub struct Siren {
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

    type Inputs = U1;

    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        // Smooth modulation to avoid clicks
        let a_raw = input[0];
        let dt = self.sample_duration.max(1.0 / DEFAULT_SR as f32);
        let tau = 0.02; // ~20 ms smoothing
        let alpha = dt / (tau + dt);
        let a_s = self.prev_a + alpha * (a_raw - self.prev_a);
        self.prev_a = a_s;

        // Advance normalized time
        self.time += dt;

        // Phi model: phi(t) = t - a(t) * sin(2 t)
        let phi = self.time - a_s * (2.0 * self.time).sin();

        // Wrap phase argument to avoid numerical growth
        let mut arg = phi * self.freq * std::f32::consts::TAU;
        let two_pi = std::f32::consts::TAU;
        if arg > two_pi || arg < -two_pi {
            arg %= two_pi;
        }

        let output = arg.sin();
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

pub fn siren() -> An<Siren> {
    let mut siren = Siren {
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
