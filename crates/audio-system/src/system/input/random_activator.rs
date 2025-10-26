use common::NodeKey;
use fastrand::Rng;
use fundsp::buffer::{BufferMut, BufferRef};
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::signal::SignalFrame;
use std::collections::HashMap;
use std::sync::Arc;

use crate::util::hash_str;

/// Random activator node ID for debugging
const RANDOM_ACTIVATOR_ID: u64 = hash_str(concat!(module_path!(), "::RandomActivator"));

/// Update interval in samples (at 44.1kHz, ~100 samples = ~2.3ms)
const UPDATE_INTERVAL_SAMPLES: usize = 4096;

/// Random activator that generates activation values directly
#[derive(Clone)]
pub struct RandomActivator {
    /// Activation controls mapped by sensor key
    activation_controls: HashMap<NodeKey, Shared>,

    /// Random number generator
    rng: Arc<parking_lot::Mutex<Rng>>,

    /// Sample counter for update timing
    sample_counter: Arc<parking_lot::Mutex<usize>>,

    /// Current activation values (for smooth transitions)
    current_activations: Arc<parking_lot::Mutex<HashMap<NodeKey, f32>>>,

    /// Target activation values
    target_activations: Arc<parking_lot::Mutex<HashMap<NodeKey, f32>>>,

    /// Smoothing factor for transitions
    smoothing: f32,

    /// Internal iteration counter
    iteration: usize,
}

impl RandomActivator {
    /// Create a new random activator with the given activation controls
    pub fn new(activation_controls: HashMap<NodeKey, Shared>) -> Self {
        let keys: Vec<NodeKey> = activation_controls.keys().copied().collect();

        let mut current = HashMap::new();
        let mut target = HashMap::new();

        for key in &keys {
            current.insert(*key, 0.0);
            target.insert(*key, 0.0);
        }

        log::info!(
            "RandomActivator::new: created with {} activation controls",
            activation_controls.len()
        );

        Self {
            activation_controls,
            rng: Arc::new(parking_lot::Mutex::new(Rng::new())),
            sample_counter: Arc::new(parking_lot::Mutex::new(0)),
            current_activations: Arc::new(parking_lot::Mutex::new(current)),
            target_activations: Arc::new(parking_lot::Mutex::new(target)),
            smoothing: 0.95, // Smooth transitions between random values
            iteration: 0,
        }
    }

    /// Apply slow-growth activation function using x^3 curve
    fn slow_growth_activation(x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }

        // Use x^3 for slow growth
        // This gives:
        // x=0.1 -> 0.001
        // x=0.2 -> 0.008
        // x=0.3 -> 0.027
        // x=0.5 -> 0.125
        // x=0.7 -> 0.343
        // x=0.9 -> 0.729
        x * x * x
    }

    /// Generate new random target activations
    fn generate_random_targets(&self) {
        let mut rng = self.rng.lock();
        let mut targets = self.target_activations.lock();

        for (key, target) in targets.iter_mut() {
            *target = if self.iteration.is_multiple_of(2) != key.idx().is_multiple_of(2) {
                // Generate random value with bias towards lower values
                // Use multiple random samples to create a distribution
                let r1 = rng.f32();
                let r2 = rng.f32();
                let r3 = rng.f32();

                // Average creates a more centered distribution
                let avg = (r1 + r2 + r3) / 3.0;

                // Apply bias towards lower values (square for stronger bias)
                let biased = avg * avg;
                Self::slow_growth_activation(biased)
            } else {
                rng.f32()
            };

            if log::log_enabled!(log::Level::Trace) && *target > 0.01 {
                log::trace!(
                    "RandomActivator: generated target for key {:?}: {:.3}",
                    key,
                    target
                );
            }
        }
    }

    /// Update activation controls with smoothed values
    fn update_activations(&self) {
        let mut current = self.current_activations.lock();
        let targets = self.target_activations.lock();

        for (key, control) in &self.activation_controls {
            if let (Some(target), Some(curr)) = (targets.get(key), current.get_mut(key)) {
                // Smooth transition to target
                *curr = *curr * self.smoothing + *target * (1.0 - self.smoothing);

                // Update the control
                control.set_value(*curr);

                if log::log_enabled!(log::Level::Trace) && *curr > 0.01 {
                    log::trace!(
                        "RandomActivator: updated key {:?}: current={:.3}, target={:.3}",
                        key,
                        curr,
                        target
                    );
                }
            }
        }
    }
}

impl AudioUnit for RandomActivator {
    fn inputs(&self) -> usize {
        1 // Takes input but doesn't use it
    }

    fn outputs(&self) -> usize {
        0 // No audio output, only updates controls
    }

    fn tick(&mut self, input: &[f32], _output: &mut [f32]) {
        // Ignore input audio (it's just random noise anyway)
        let _ = input;

        // Update sample counter
        let mut counter = self.sample_counter.lock();
        *counter += 1;

        // Generate new targets periodically
        if *counter >= UPDATE_INTERVAL_SAMPLES {
            *counter = 0;
            drop(counter); // Release lock before generating

            self.iteration += 1;
            self.generate_random_targets();
        }

        // Always update activations (for smooth transitions)
        self.update_activations();
    }

    fn process(&mut self, size: usize, input: &BufferRef, _output: &mut BufferMut) {
        // Process in chunks similar to tick
        for i in 0..size {
            let in_sample = if input.channels() > 0 && i < size {
                [input.at(0, i).to_array()[0]]
            } else {
                [0.0]
            };

            let mut out_sample = []; // No output
            self.tick(&in_sample, &mut out_sample);
        }
    }

    fn set_sample_rate(&mut self, _rate: f64) {
        // Sample rate doesn't affect random generation
    }

    fn reset(&mut self) {
        // Reset all activations to 0
        let mut current = self.current_activations.lock();
        let mut targets = self.target_activations.lock();

        for (_, val) in current.iter_mut() {
            *val = 0.0;
        }
        for (_, val) in targets.iter_mut() {
            *val = 0.0;
        }

        // Update controls to 0
        for control in self.activation_controls.values() {
            control.set_value(0.0);
        }

        // Reset counter
        *self.sample_counter.lock() = 0;

        log::debug!("RandomActivator: reset all activations to 0");
    }

    fn allocate(&mut self) {
        // No allocation needed
    }

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        // No routing needed - return empty signal frame
        SignalFrame::new(self.outputs())
    }

    fn get_id(&self) -> u64 {
        RANDOM_ACTIVATOR_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_activator_creation() {
        let mut controls = HashMap::new();
        let key = NodeKey::new(0, 0);
        let control = shared(0.0);
        controls.insert(key, control.clone());

        let activator = RandomActivator::new(controls);
        assert_eq!(activator.activation_controls.len(), 1);
        assert_eq!(control.value(), 0.0);
    }

    #[test]
    fn test_random_activation_generation() {
        let mut controls = HashMap::new();
        for i in 0..3 {
            let key = NodeKey::new(0, i);
            let control = shared(0.0);
            controls.insert(key, control);
        }

        let activator = RandomActivator::new(controls.clone());

        // Generate random targets
        activator.generate_random_targets();

        // Check that targets were generated
        let targets = activator.target_activations.lock();
        assert_eq!(targets.len(), 3);

        for (_, &value) in targets.iter() {
            // Values should be in [0, 1] range
            assert!((0.0..=1.0).contains(&value));
        }
    }

    #[test]
    fn test_slow_growth_consistency() {
        // Test that our implementation matches the FFTAnalyzer version
        for x in [0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0] {
            let result = RandomActivator::slow_growth_activation(x);
            assert!((0.0..=1.0).contains(&result));
            if x > 0.0 && x < 1.0 {
                // Should be less than linear
                assert!(result < x);
            }
        }
    }
}
