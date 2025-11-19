use common::NodeKey;
use fastrand::Rng;
use fundsp::buffer::{BufferMut, BufferRef};
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::signal::SignalFrame;
use std::ops::{Add, Div, Sub, SubAssign};
use std::sync::Arc;
use std::{collections::HashMap, ops::AddAssign};

use crate::util::{hash_str, S};

/// Random excitor node ID for debugging
const RANDOM_EXCITOR_ID: u64 = hash_str(concat!(module_path!(), "::RandomExcitor"));

const MAX_DURATION_BASE_S: S = 17.5;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
/// Duration in milliseconds stored as float
///
/// Either `f32` or `f64` is used depending on the feature flag.
struct Duration(S);

#[allow(clippy::unnecessary_cast)]
impl Duration {
    const ZERO: Self = Duration(0.0);

    fn from_sample_rate(rate_per_second: f64) -> Self {
        Self((1000.0 / rate_per_second) as S)
    }

    fn from_secs(secs: S) -> Self {
        Self(secs * 1000.0)
    }
}

impl Add for Duration {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl AddAssign for Duration {
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0;
    }
}

impl Sub for Duration {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl SubAssign for Duration {
    fn sub_assign(&mut self, other: Self) {
        self.0 -= other.0;
    }
}

impl Div for Duration {
    type Output = isize;

    fn div(self, rhs: Self) -> Self::Output {
        (self.0 / rhs.0).round() as isize
    }
}

impl Div<S> for Duration {
    type Output = Duration;

    fn div(self, rhs: S) -> Self::Output {
        Self(self.0 / rhs)
    }
}

/// Random excitor that generates excitement values directly
#[derive(Clone)]
pub struct RandomExcitor {
    /// Excitement controls mapped by sensor key
    excitement_controls: HashMap<NodeKey, Shared>,

    /// Random number generator
    rng: Arc<parking_lot::Mutex<Rng>>,

    /// Current excitement values (for smooth transitions)
    current: Arc<parking_lot::Mutex<HashMap<NodeKey, (S, Duration)>>>,

    /// Target excitement values
    target: Arc<parking_lot::Mutex<HashMap<NodeKey, (S, Duration)>>>,

    /// Current sample rate
    sample_rate: f64,
}

#[allow(clippy::unnecessary_cast)]
impl RandomExcitor {
    /// Create a new random excitor with the given excitement controls
    pub fn new(excitement_controls: HashMap<NodeKey, Shared>) -> Self {
        let keys: Vec<NodeKey> = excitement_controls.keys().copied().collect();

        let mut current = HashMap::with_capacity(keys.len());
        let mut target = HashMap::with_capacity(keys.len());

        for key in &keys {
            current.insert(*key, (0.0, Duration::ZERO));
            target.insert(*key, (0.0, Duration::ZERO));
        }

        log::info!(
            "RandomExcitor::new: created with {} excitement controls",
            excitement_controls.len()
        );

        Self {
            excitement_controls,
            rng: Arc::new(parking_lot::Mutex::new(Rng::new())),
            current: Arc::new(parking_lot::Mutex::new(current)),
            target: Arc::new(parking_lot::Mutex::new(target)),
            sample_rate: 44100.0,
        }
    }

    /// Create a new RandomExcitor with a given seed
    pub fn new_seeded(seed: u64, excitement_controls: HashMap<NodeKey, Shared>) -> Self {
        let node = Self::new(excitement_controls);

        node.rng.lock().seed(seed);

        node
    }

    /// Compute duration for one frame with current sample rate
    fn frame_duration(&self) -> Duration {
        Duration::from_sample_rate(self.sample_rate)
    }

    /// Compute the number of frames between two durations
    fn frames_between(&self, from: &Duration, to: &Duration) -> isize {
        let frame_duration = self.frame_duration();
        let delta = *to - *from;
        delta / frame_duration
    }

    /// Generate new random target excitements
    fn generate_random_target(&self, key: NodeKey) -> (S, Duration) {
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

        let duration = Duration::from_secs(avg.powi(3) * MAX_DURATION_BASE_S);

        let idx = key.idx();
        let d = rng.usize(1..idx);

        let mut value = if d.is_multiple_of(3) && d.is_multiple_of(5) {
            avg.sqrt()
        } else if d.is_multiple_of(5) {
            r2.powi(3)
        } else if d.is_multiple_of(3) {
            r3.powi(5)
        } else {
            0.0
        };

        if !value.is_normal() && value != 0.0 {
            value = 0.0;
        }

        (value, duration)
    }

    /// Update excitement controls with smoothed values
    fn advance_frame(&self) {
        let mut current = self.current.lock();
        let targets = self.target.lock();
        let frame_duration = self.frame_duration();

        for (key, control) in &self.excitement_controls {
            if let (Some((target, target_duration)), Some((curr, elapsed))) =
                (targets.get(key), current.get_mut(key))
            {
                let remaining_frames = self.frames_between(elapsed, target_duration);
                if remaining_frames > 0 {
                    let remaining_value = *target - *curr;
                    let increment = remaining_value / remaining_frames as S;
                    *curr += increment;
                    *elapsed += frame_duration;

                    // Update the control
                    control.set_value(*curr as f32);
                }
            }
        }
    }

    fn renew(&self) {
        let mut current = self.current.lock();
        let mut targets = self.target.lock();
        let frame_duration = self.frame_duration();
        for (key, target) in targets.iter_mut() {
            if let Some(curr) = current
                .get_mut(key)
                .filter(|(_, elapsed)| (target.1 - *elapsed) < frame_duration)
            {
                *target = self.generate_random_target(*key);
                curr.1 = Duration::ZERO;
            }
        }

        // avoid long silence
        if targets.values().all(|&(t, _)| t == 0.0) {
            for target in targets.values_mut() {
                target.1 = target.1 / 3.0;
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
        self.renew();
    }

    fn process(&mut self, size: usize, _input: &BufferRef, _output: &mut BufferMut) {
        (0..size).for_each(|_| self.advance_frame());
        self.renew();
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate;
    }

    fn reset(&mut self) {
        // Reset all excitements to 0
        let mut current = self.current.lock();
        let mut targets = self.target.lock();

        for (_, val) in current.iter_mut() {
            *val = (0.0, Duration::ZERO);
        }
        for (_, val) in targets.iter_mut() {
            *val = (0.0, Duration::ZERO);
        }

        // Update controls to 0
        for control in self.excitement_controls.values() {
            control.set_value(0.0);
        }

        // Reset counter
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
        let control = shared(0.0);
        controls.insert(key, control.clone());

        let excitor = RandomExcitor::new_seeded(42, controls);
        assert_eq!(excitor.excitement_controls.len(), 1);
        assert_eq!(control.value(), 0.0);
    }

    #[test]
    fn test_random_excitement_generation() {
        let mut controls = HashMap::new();
        for i in 0..3 {
            let key = NodeKey::new(0, i);
            let control = shared(0.0);
            controls.insert(key, control);
        }

        let excitor = RandomExcitor::new(controls.clone());

        // Generate random targets
        {
            let mut targets = excitor.target.lock();
            for key in controls.keys() {
                targets.insert(*key, excitor.generate_random_target(*key));
            }
        }

        // Check that targets were generated
        let targets = excitor.target.lock();
        assert_eq!(targets.len(), 3);

        for (_, &(value, _duration)) in targets.iter() {
            // Values should be in [0, 1] range
            assert!((0.0..=1.0).contains(&value));
        }
    }
}
