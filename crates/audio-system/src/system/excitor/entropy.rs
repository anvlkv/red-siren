use std::collections::HashMap;

use common::NodeKey;
use fundsp::{
    buffer::{BufferMut, BufferRef},
    prelude::*,
    signal::SignalFrame,
};

use crate::system::excitor::control::Control;
use crate::util::hash_str;

const ENTROPY_DRIVER_ID: u64 = hash_str(concat!(module_path!(), "::EntropyDriver"));
const SIMD_LEN: usize = 8; // f32x8 lanes

/// Pseudo-random excitement driver.
///
/// Passes audio through unchanged (1-in / 1-out) and, approximately every 250 ms,
/// drives every registered [`Control`] with a fresh LCG-derived (re, im) pair so
/// that the siren strings are continuously excited without requiring mic input.
#[derive(Clone)]
pub struct EntropyDriver {
    controls: Vec<Control>,
    interval_samples: u64,
    counter: u64,
    rng: u64,
}

impl EntropyDriver {
    pub fn new(excitements: HashMap<NodeKey, Control>, sample_rate: f64) -> Self {
        let interval_samples = std::cmp::Ord::max((sample_rate * 0.25) as u64, 1);
        let controls: Vec<Control> = excitements.into_values().collect();
        Self {
            controls,
            interval_samples,
            counter: 0,
            rng: 0xdeadbeef_cafef00d,
        }
    }

    fn lcg_next(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.rng >> 33) as f32 / (u32::MAX as f32)
    }

    fn drive(&mut self) {
        // Use index-based iteration to avoid a simultaneous mutable borrow of
        // `self` (from `lcg_next`) while also holding an immutable borrow of
        // `self.controls` (from a for-each iterator).
        for i in 0..self.controls.len() {
            let re = self.lcg_next();
            let im = self.lcg_next() * 0.3;
            self.controls[i].set_value((re, im));
        }
    }
}

impl AudioUnit for EntropyDriver {
    fn inputs(&self) -> usize {
        1
    }

    fn outputs(&self) -> usize {
        1
    }

    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        output[0] = input[0];
        self.counter += 1;
        if self.counter >= self.interval_samples {
            self.counter = 0;
            self.drive();
        }
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Pass audio through — each SIMD slot holds SIMD_LEN (8) scalar samples.
        // `size` is a scalar sample count; `at`/`at_mut` take a SIMD frame index.
        let simd_frames = size / SIMD_LEN;
        for s in 0..simd_frames {
            *output.at_mut(0, s) = input.at(0, s);
        }
        // Handle any remaining scalar samples (when size is not divisible by SIMD_LEN).
        for j in (simd_frames * SIMD_LEN)..size {
            output.set_f32(0, j, input.at_f32(0, j));
        }
        // `size` is already the scalar sample count — no multiplication needed.
        self.counter += size as u64;
        if self.counter >= self.interval_samples {
            self.counter %= self.interval_samples;
            self.drive();
        }
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.interval_samples = std::cmp::Ord::max((rate * 0.25) as u64, 1);
    }

    fn reset(&mut self) {
        self.counter = 0;
    }

    fn allocate(&mut self) {}

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        SignalFrame::new(1)
    }

    fn get_id(&self) -> u64 {
        ENTROPY_DRIVER_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_driver(n: usize) -> EntropyDriver {
        let controls: HashMap<NodeKey, Control> = (0u8..n as u8)
            .map(|i| (NodeKey::new(0, i), Control::default()))
            .collect();
        EntropyDriver::new(controls, 44100.0)
    }

    #[test]
    fn inputs_and_outputs() {
        let d = make_driver(3);
        assert_eq!(d.inputs(), 1);
        assert_eq!(d.outputs(), 1);
    }

    #[test]
    fn tick_passes_audio_through() {
        let mut d = make_driver(0);
        let mut out = [0.0_f32];
        d.tick(&[0.5], &mut out);
        assert!((out[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn drive_does_not_panic_with_controls() {
        let mut d = make_driver(4);
        d.drive();
    }

    #[test]
    fn lcg_produces_values_in_unit_range() {
        let mut d = make_driver(0);
        for _ in 0..1000 {
            let v = d.lcg_next();
            assert!(v >= 0.0 && v <= 1.0, "lcg_next out of [0, 1]: {v}");
        }
    }
}
