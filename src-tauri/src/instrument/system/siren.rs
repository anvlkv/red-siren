use fundsp::hacker32::prelude::*;

const fn hash_str(s: &str) -> u64 {
    // FNV-1a hash algorithm (const-friendly)
    let mut hash = 0xcbf29ce484222325u64;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        i += 1;
    }
    hash
}

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));
const SIREN_BASE_HZ: f32 = 0.5;

#[derive(Clone)]
pub struct Siren {
    a_var: Var,
    freq: f32,
    phase: f32,
    sample_duration: f32,
}
//  let phi: f32 = t - a * sin(2.0 * t);
// sin(phi * SIREN_BASE_HZ * std::f32::consts::TAU)
impl AudioNode for Siren {
    const ID: u64 = SIREN_ID;

    type Inputs = U0;

    type Outputs = U1;

    fn tick(&mut self, _input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        // Get the current modulation value from the shared variable
        let a = self.a_var.value();

        // Calculate phi using the formula: phi = t - a * sin(2.0 * t)
        let t = self.phase;
        let phi = t - a * sin(2.0 * t);

        // Calculate output using sin(phi * SIREN_BASE_HZ * TAU)
        let output = sin(phi * self.freq * std::f32::consts::TAU);

        // Advance phase by one sample duration
        self.phase += self.sample_duration;

        [output].into()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
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
        sample_duration: 1.0 / DEFAULT_SR as f32,
    };
    siren.reset();
    siren.set_sample_rate(DEFAULT_SR);
    An(siren)
}
