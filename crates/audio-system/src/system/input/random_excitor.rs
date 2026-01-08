use common::NodeKey;
use fastrand::Rng;
use fundsp::buffer::{BufferMut, BufferRef};
use fundsp::prelude::*;
use fundsp::signal::SignalFrame;
use std::collections::HashMap;
use std::sync::Arc;

use crate::system::input::adsr::Adsr;
use crate::util::{hash_str, SComplex, S};
use crate::ExcitementControl;

/// Random excitor node ID for debugging
const RANDOM_EXCITOR_ID: u64 = hash_str(concat!(module_path!(), "::RandomExcitor"));

/// Upper bound for random hold duration in seconds used to schedule new targets
const MAX_DURATION_BASE_S: S = 17.5;
const MIN_HOLD_SECS: S = 0.1;
const DURATION_POWER: S = 3.0; // 2.0 for more responsive, 4.0 for more sustained
const SILENCE_THRESH: S = 1.0e-9;
const SILENCE_HYST_FRAMES: u64 = 64;

/// Random excitor that generates excitement values via Adsr envelopes
#[derive(Clone)]
pub struct RandomExcitor {
    /// Excitement controls mapped by sensor key
    excitement_controls: HashMap<NodeKey, ExcitementControl>,

    /// Per-key envelopes
    envelopes: HashMap<NodeKey, (Adsr, SComplex)>,

    /// Per-key frames remaining until next target refresh
    remaining_frames: HashMap<NodeKey, u64>,

    /// Per-key hysteresis counter for silence detection
    silent_frames: HashMap<NodeKey, u64>,

    /// Random number generator
    rng: Arc<parking_lot::Mutex<Rng>>,

    /// Current sample rate
    sample_rate: f64,
}

impl RandomExcitor {
    /// Create a new random excitor with the given excitement controls
    pub fn new(excitement_controls: HashMap<NodeKey, ExcitementControl>) -> Self {
        let mut envelopes = HashMap::with_capacity(excitement_controls.len());
        let mut remaining_frames = HashMap::with_capacity(excitement_controls.len());

        // Default sample rate; will be overridden by host
        let sample_rate = 44100.0;

        for (key, control) in &excitement_controls {
            let mut env = (Adsr::new(sample_rate), SComplex::ZERO);
            // Initialize control and envelope to zero
            control.reset();
            // Kick envelope into Idle aligned with zero target
            env.0.update(SComplex::ZERO, SComplex::ZERO);
            envelopes.insert(*key, env);
            remaining_frames.insert(*key, 0);
        }

        let mut silent_frames = HashMap::with_capacity(excitement_controls.len());
        for key in excitement_controls.keys() {
            silent_frames.insert(*key, 0);
        }

        log::info!(
            "RandomExcitor::new: created with {} excitement controls",
            excitement_controls.len()
        );

        Self {
            excitement_controls,
            envelopes,
            remaining_frames,
            silent_frames,
            rng: Arc::new(parking_lot::Mutex::new(Rng::new())),
            sample_rate,
        }
    }

    /// Create a new RandomExcitor with a given seed
    pub fn new_seeded(seed: u64, excitement_controls: HashMap<NodeKey, ExcitementControl>) -> Self {
        let node = Self::new(excitement_controls);
        node.rng.lock().seed(seed);
        node
    }

    /// Compute frames count from seconds at current sample rate
    fn frames_from_secs(&self, secs: S) -> u64 {
        (secs.max(0.0) * self.sample_rate as S).ceil() as u64
    }

    /// Generate a new random target excitement and suggested hold duration (seconds)
    fn generate_random_target(&self, key: NodeKey) -> (SComplex, S) {
        let mut rng = self.rng.lock();

        let mut r = {
            let mut rng = rng.fork();
            move || {
                #[cfg(feature = "hi_fi")]
                let val = rng.f64();
                #[cfg(not(feature = "hi_fi"))]
                let val = rng.f32();
                val
            }
        };

        // Use multiple random samples to create a distribution
        let r1 = r();
        let r2 = r();
        let r3 = r();

        let avg = (r1 + r2 + r3) / 3.0;

        // Suggested duration uses configurable power to shape distribution, scaled by MAX_DURATION_BASE_S
        let mut duration_secs = avg.powf(DURATION_POWER).clamp(0.0, 1.0) * MAX_DURATION_BASE_S;
        // Clamp to min/max to avoid overly twitchy or excessively long holds
        duration_secs = duration_secs.clamp(MIN_HOLD_SECS, MAX_DURATION_BASE_S);

        let idx = key.idx();
        let d = rng.usize(1..std::cmp::max(idx, 2)); // avoid empty range
        let im = rng
            .choice([avg.powi(3), r1, r2, r3, avg])
            .unwrap_or(S::EPSILON);

        let mut value = if d.is_multiple_of(3) && d.is_multiple_of(5) {
            SComplex::new(avg.sqrt(), im)
        } else if d.is_multiple_of(5) {
            SComplex::new(r2.powi(3), im)
        } else if d.is_multiple_of(3) {
            SComplex::new(r3.powi(5), im)
        } else {
            SComplex::ZERO
        };

        if !value.is_normal() {
            value = SComplex::ZERO;
        }

        (value, duration_secs)
    }

    /// Refresh per-key targets by updating envelopes when their hold period expires
    fn maybe_refresh_targets(&mut self) {
        // Collect keys that need refresh first to avoid borrowing conflicts
        let keys_to_refresh: Vec<NodeKey> = self
            .remaining_frames
            .iter()
            .filter_map(|(key, frames_left)| if *frames_left == 0 { Some(*key) } else { None })
            .collect();

        for key in keys_to_refresh {
            let (next_target, duration_secs) = self.generate_random_target(key);
            if let Some((env, prev)) = self.envelopes.get_mut(&key) {
                env.update(*prev, next_target);
                *prev = next_target;
            }

            let frames = std::cmp::max(self.frames_from_secs(duration_secs), 1);
            if let Some(frames_left) = self.remaining_frames.get_mut(&key) {
                *frames_left = frames;
            }
        }

        // Avoid long silence with hysteresis: only shorten holds if all channels stayed below threshold for N frames
        let all_hysteresis_met = self
            .silent_frames
            .values()
            .all(|&f| f >= SILENCE_HYST_FRAMES);
        if all_hysteresis_met {
            for frames_left in self.remaining_frames.values_mut() {
                *frames_left = std::cmp::max(*frames_left / 3, 1);
            }
            // Reset hysteresis counters after adjustment
            for f in self.silent_frames.values_mut() {
                *f = 0;
            }
        }
    }

    /// Advance one audio frame: tick envelopes and push values to controls; decrement hold counters
    #[allow(clippy::unnecessary_cast)]
    fn advance_frame(&mut self) {
        for (key, control) in &self.excitement_controls {
            if let Some((env, _)) = self.envelopes.get_mut(key) {
                let v = env.tick();
                control.set_value(v);

                // Update per-key silence hysteresis counter
                if let Some(s) = self.silent_frames.get_mut(key) {
                    if v.0 <= SILENCE_THRESH {
                        *s = s.saturating_add(1);
                    } else {
                        *s = 0;
                    }
                }

                if let Some(frames_left) = self.remaining_frames.get_mut(key) {
                    *frames_left = frames_left.saturating_sub(1);
                }
            }
        }
    }
}

impl AudioUnit for RandomExcitor {
    fn inputs(&self) -> usize {
        1 // Takes input but doesn't use it
    }

    fn outputs(&self) -> usize {
        0 // No audio output, only updates controls
    }

    fn tick(&mut self, _input: &[f32], _output: &mut [f32]) {
        self.advance_frame();
        self.maybe_refresh_targets();
    }

    fn process(&mut self, size: usize, _input: &BufferRef, _output: &mut BufferMut) {
        for _ in 0..size {
            self.advance_frame();
            self.maybe_refresh_targets();
        }
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate;
        for (env, _) in self.envelopes.values_mut() {
            env.update_sample_rate(rate);
        }
    }

    fn reset(&mut self) {
        // Reset controls and envelopes to zero
        for control in self.excitement_controls.values() {
            control.reset();
        }
        for env in self.envelopes.values_mut() {
            // Reinitialize to Idle/zero
            *env = (Adsr::new(self.sample_rate), SComplex::ZERO);
            env.0.update(SComplex::ZERO, SComplex::ZERO);
        }
        for frames_left in self.remaining_frames.values_mut() {
            *frames_left = 0;
        }

        log::debug!("RandomExcitor: reset all excitements to 0");
    }

    fn allocate(&mut self) {
        // No allocation needed
    }

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        // No routing needed - return empty signal frame
        SignalFrame::new(self.outputs())
    }

    fn get_id(&self) -> u64 {
        RANDOM_EXCITOR_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_excitor_creation() {
        let mut controls = HashMap::new();
        let key = NodeKey::new(0, 0);
        let control = ExcitementControl::default();
        controls.insert(key, control.clone());

        let excitor = RandomExcitor::new_seeded(42, controls);
        assert_eq!(excitor.excitement_controls.len(), 1);
        assert_eq!(control.value().re, 0.0);
    }

    #[test]
    fn test_random_excitor_drives_envelopes() {
        let mut controls = HashMap::new();
        for i in 0..3 {
            let key = NodeKey::new(0, i);
            let control = ExcitementControl::default();
            controls.insert(key, control);
        }

        let mut excitor = RandomExcitor::new_seeded(7, controls.clone());
        excitor.set_sample_rate(48000.0);

        // Run a few ticks to allow envelopes to produce values
        for _ in 0..256 {
            excitor.tick(&[], &mut []);
        }

        // Values should remain in [0,1]
        for control in controls.values() {
            let v = control.value();
            assert!((0.0..1.0).contains(&v.re), "value out of bounds: {v}");
        }
    }
}
