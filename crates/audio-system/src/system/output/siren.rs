use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const SIREN_ID: u64 = hash_str(concat!(module_path!(), "::Siren"));
const SIREN_BASE_HZ: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SirenState {
    Idle,
    Active,
    Tail,
}

#[derive(Clone)]
pub struct Siren {
    freq: f32,
    phase: f32,
    sample_duration: f32,
    last_sine: f32,
    last_a: f32,
    state: SirenState,
    epsilon: f32,
    tail_dir_sign: f32,
}

impl AudioNode for Siren {
    const ID: u64 = SIREN_ID;

    type Inputs = U1;

    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        // Raw gate/time coefficient (may be negative => backwards)
        let a = input[0];

        // Current sine sample
        let s = self.phase.sin();

        // Zero-cross boundary detection
        let crossed = (s * self.last_sine) < 0.0 || s.abs() < self.epsilon;

        // Output and state transitions
        let mut out = 0.0f32;

        match self.state {
            SirenState::Idle => {
                // Silent and frozen. Start only when a > 0 at boundary.
                if a > 0.0 {
                    self.state = SirenState::Active;
                }
            }
            SirenState::Active => {
                // Emit sine. If a <= 0, finish to next boundary (Tail).
                if crossed && a <= 0.0 {
                    // Hit boundary while non-positive gate, stop immediately.
                    self.state = SirenState::Idle;
                    self.phase = 0.0;
                    out = 0.0;
                } else {
                    out = s;

                    if a <= 0.0 {
                        self.state = SirenState::Tail;
                        // Preserve direction of travel for the tail.
                        self.tail_dir_sign = if self.last_a >= 0.0 { 1.0 } else { -1.0 };
                    }
                }
            }
            SirenState::Tail => {
                // Keep emitting until next zero crossing, then stop.
                if crossed {
                    // Tail complete, transition to idle without emitting
                    self.state = SirenState::Idle;
                    self.phase = 0.0;
                    out = 0.0;
                } else {
                    // Continue tail emission
                    out = s;
                }
            }
        }

        // Phase/time advancement
        let two_pi = std::f32::consts::TAU;
        let omega = two_pi * self.freq;

        match self.state {
            SirenState::Idle => {
                // Do not advance when muted.
            }
            SirenState::Active => {
                // Advance with 'a' coefficient (sign preserved for backwards motion).
                self.phase += omega * self.sample_duration * a;
                if self.phase >= two_pi || self.phase < 0.0 {
                    self.phase %= two_pi;
                    if self.phase < 0.0 {
                        self.phase += two_pi;
                    }
                }
            }
            SirenState::Tail => {
                // Finish to boundary in preserved direction.
                self.phase += omega * self.sample_duration * self.tail_dir_sign;
                if self.phase >= two_pi || self.phase < 0.0 {
                    self.phase %= two_pi;
                    if self.phase < 0.0 {
                        self.phase += two_pi;
                    }
                }
            }
        }

        // Bookkeeping
        self.last_sine = s;
        self.last_a = a;

        [out].into()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.last_sine = 0.0;
        self.last_a = 0.0;
        self.state = SirenState::Idle;
        self.tail_dir_sign = 1.0;
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_duration = 1.0 / sample_rate as f32;
    }
}

pub fn siren() -> An<Siren> {
    let mut siren = Siren {
        freq: SIREN_BASE_HZ,
        phase: 0.0,
        sample_duration: 1.0 / DEFAULT_SR as f32,
        last_sine: 0.0,
        last_a: 0.0,
        state: SirenState::Idle,
        epsilon: 1.0e-6,
        tail_dir_sign: 1.0,
    };
    siren.reset();
    siren.set_sample_rate(DEFAULT_SR);
    An(siren)
}
