use std::{f32, iter};

use fundsp::prelude::*;
use num_complex::Complex;

mod generator;
mod phase;
mod scheme;
mod shape;

use phase::Phase;
use scheme::Scheme;
use shape::Shape;

#[derive(Clone)]
pub struct QueuingAdsr<S: Real + Float, A: AudioNode> {
    shape: Shape<S>,
    phaser: Vec<Phase<S>>,
    table: Wavetable,
    generator: A,
    inputs: Frame<f32, A::Inputs>,
    base_scheme: Scheme,
    sample_rate: u64,
}

impl<S: Real + Float, A: AudioNode> QueuingAdsr<S, A> {
    pub fn new(
        sample_rate: f64,
        mut generator: A,
        table_duration_s: f64,
        min_pitch: f64,
        max_pitch: f64,
        tables_per_octave: f64,
    ) -> Self {
        let table_len = (table_duration_s * sample_rate).ceil() as usize;
        let inputs = Frame::splat(0.0);
        let wave = Vec::from_iter(
            iter::from_fn(|| generator.tick(&inputs).into_iter().next()).take(table_len),
        );
        let table = Wavetable::from_wave(min_pitch, max_pitch, tables_per_octave, &wave);
        Self {
            generator,
            table,
            inputs,
            shape: Shape::zero(),
            phaser: Vec::new(),
            base_scheme: Scheme::default(),
            sample_rate: sample_rate as u64,
        }
    }

    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        // let d = self.sample_rate as f64 / sample_rate;
        // let old_scheme = self.base_scheme;
        // self.sample_rate = sample_rate as u64;
        // self.base_scheme.atack = (self.base_scheme.atack as f64 * d) as u64;
        // self.base_scheme.decay = (self.base_scheme.decay as f64 * d) as u64;
        // self.base_scheme.sustain = (self.base_scheme.sustain as f64 * d) as u64;
        // self.base_scheme.release = (self.base_scheme.release as f64 * d) as u64;
        // self.phase
        //     .update_sample_rate(d, &self.base_scheme, &old_scheme);
    }

    pub fn update(&mut self, prev: Complex<S>, next: Complex<S>) {
        todo!()
    }

    pub fn tick(&mut self) -> (S, S) {
        todo!()
        // let target = self.target_value();
        // let steps = self.phase.remaining_steps();
        // let increment = if steps > 0 {
        //     (target - self.shape.running_value) / S::from_f32(steps as f32)
        // } else {
        //     S::zero()
        // };

        // let primary = if let Some(next_phase) = self.phase.tick() {
        //     self.phase = next_phase;
        //     self.shape.running_value += increment;
        //     self.shape.running_value
        // } else {
        //     self.phase = self
        //         .phase
        //         .next_phase(&self.base_scheme, self.shape.running_value);
        //     self.shape.running_value = target;
        //     self.shape.running_value
        // };

        // let secondary = self.shape.control_a * primary;

        // (primary, secondary)
    }
}

pub fn mount_adsr_an<A>(net: &mut Net, inner: A) -> NodeId
where
    A: AudioNode,
{
    todo!();
}

#[cfg(test)]
mod tests {
    use super::*;

    type S = f32;

    const SR: f64 = 48_000.0;
    const EPS: S = 1.0e-6;

    #[test]
    fn tick_idle_returns_zero() {
        let mut env = QueuingAdsr::<S>::new(SR);
        let (v, _) = env.tick();
        assert!(
            (v - 0.0).abs() <= EPS,
            "Idle tick should return zero, got {}",
            v
        );
    }

    #[test]
    fn attack_reaches_target_one() {
        let mut env = QueuingAdsr::<S>::new(SR);
        // Demand an upward move to 1.0
        env.update(Complex::<S>::ZERO, Complex::<S>::ONE);

        // Attack steps are approximately sample_rate when meta=1 and distance_up=1.
        // Tick slightly more than SR to ensure completion and phase transition.
        let mut has_reached_one = false;
        for _ in 0..(SR as usize + 50) {
            if (env.tick().0 - 1.0).abs() <= EPS {
                has_reached_one = true;
            }
        }
        assert!(has_reached_one, "Envelope should reach ~1.0 after attack",);
    }

    #[test]
    fn release_reaches_zero() {
        let mut env = QueuingAdsr::<S>::new(SR);
        // Go up first
        env.update(Complex::<S>::ZERO, Complex::<S>::ONE);
        let mut v = 0.0;
        for _ in 0..(SR as usize + 10) {
            v = env.tick().0;
        }
        // Now request release to zero
        env.update(Complex::<S>::new(v, v), Complex::<S>::ZERO);
        for _ in 0..(SR as usize + 10) {
            v = env.tick().0;
        }
        assert!(v <= EPS, "Envelope should release to ~0.0, got {}", v);
    }
}
