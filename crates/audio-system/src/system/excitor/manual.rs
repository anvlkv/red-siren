use common::NodeKey;
use fundsp::{
    buffer::{BufferMut, BufferRef},
    prelude::*,
    signal::SignalFrame,
};

use crate::system::excitor::control::Control;
use crate::util::hash_str;

/// Stable node ID for `ManualExcitor` used by the fundsp graph.
const MANUAL_EXCITOR_ID: u64 = hash_str(concat!(module_path!(), "::ManualExcitor"));

/// A zero-logic [`AudioUnit`] whose sole purpose is to hold [`Control`] handles
/// so that external UI code can drive excitation directly by calling
/// `control.real.set_value(...)` / `control.imaginary.set_value(...)`.
///
/// No audio is generated or consumed; the node is purely a named placeholder
/// inside a `Net` that gives the rest of the system a stable location for the
/// per-key control [`Shared`] values.
#[derive(Clone)]
pub struct ManualExcitor {
    /// One [`Control`] per node, ordered to match `keys`.
    controls: Vec<Control>,
    /// Parallel vec used for key-based lookup.
    keys: Vec<NodeKey>,
}

impl ManualExcitor {
    /// Create a new `ManualExcitor` with a default (zero) [`Control`] for every key.
    pub fn new(keys: Vec<NodeKey>) -> Self {
        let controls = keys.iter().map(|_| Control::default()).collect();
        Self { controls, keys }
    }

    /// Slice of all controls, in the same order as the keys passed to [`Self::new`].
    ///
    /// Callers may read or write values through the returned [`Control`] handles at
    /// any time without acquiring any lock — `Shared` is inherently thread-safe.
    pub fn controls(&self) -> &[Control] {
        &self.controls
    }

    /// Look up the [`Control`] handle associated with `key`, or `None` if the key
    /// was not registered at construction time.
    pub fn control_for(&self, key: &NodeKey) -> Option<&Control> {
        self.keys
            .iter()
            .position(|k| k == key)
            .map(|idx| &self.controls[idx])
    }
}

impl AudioUnit for ManualExcitor {
    /// No audio inputs — values are pushed in via [`Control`] handles externally.
    fn inputs(&self) -> usize {
        0
    }

    /// No audio outputs — this node exists solely to hold control state.
    fn outputs(&self) -> usize {
        0
    }

    /// No-op: values are driven externally via [`Control`] handles.
    fn tick(&mut self, _input: &[f32], _output: &mut [f32]) {}

    /// No-op: values are driven externally via [`Control`] handles.
    fn process(&mut self, _size: usize, _input: &BufferRef, _output: &mut BufferMut) {}

    /// No-op: sample rate is irrelevant for a passive control holder.
    fn set_sample_rate(&mut self, _rate: f64) {}

    /// Reset every control to zero.
    fn reset(&mut self) {
        for control in &self.controls {
            control.reset();
        }
    }

    /// No-op: nothing to pre-allocate.
    fn allocate(&mut self) {}

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        SignalFrame::new(0)
    }

    fn get_id(&self) -> u64 {
        MANUAL_EXCITOR_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_does_not_panic() {
        let mut excitor = ManualExcitor::new(vec![NodeKey::new(0, 0)]);
        // Must not panic even with empty slices (0 inputs / 0 outputs).
        excitor.tick(&[], &mut []);
    }

    #[test]
    fn control_for_registered_key_returns_some() {
        let key = NodeKey::new(0, 0);
        let excitor = ManualExcitor::new(vec![key]);
        assert!(
            excitor.control_for(&NodeKey::new(0, 0)).is_some(),
            "expected Some for a registered key"
        );
    }

    #[test]
    fn control_for_unregistered_key_returns_none() {
        let key = NodeKey::new(0, 0);
        let excitor = ManualExcitor::new(vec![key]);
        assert!(
            excitor.control_for(&NodeKey::new(1, 1)).is_none(),
            "expected None for an unregistered key"
        );
    }

    #[test]
    fn reset_zeroes_all_controls() {
        let key = NodeKey::new(0, 0);
        let mut excitor = ManualExcitor::new(vec![key]);

        // Drive a non-zero value through the handle before reset.
        if let Some(ctrl) = excitor.control_for(&NodeKey::new(0, 0)) {
            ctrl.real.set_value(0.5);
            ctrl.imaginary.set_value(0.75);
        }

        excitor.reset();

        if let Some(ctrl) = excitor.control_for(&NodeKey::new(0, 0)) {
            assert_eq!(ctrl.real.value(), 0.0, "real should be zero after reset");
            assert_eq!(
                ctrl.imaginary.value(),
                0.0,
                "imaginary should be zero after reset"
            );
        } else {
            panic!("control_for returned None unexpectedly");
        }
    }

    #[test]
    fn inputs_and_outputs_are_zero() {
        let excitor = ManualExcitor::new(vec![NodeKey::new(0, 0)]);
        assert_eq!(excitor.inputs(), 0);
        assert_eq!(excitor.outputs(), 0);
    }

    #[test]
    fn controls_slice_length_matches_keys() {
        let keys = vec![NodeKey::new(0, 0), NodeKey::new(0, 1), NodeKey::new(1, 0)];
        let excitor = ManualExcitor::new(keys.clone());
        assert_eq!(excitor.controls().len(), keys.len());
    }
}
