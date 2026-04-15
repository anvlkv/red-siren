use std::sync::Arc;

use fundsp::{
    buffer::{BufferMut, BufferRef},
    net::Net,
    prelude::*,
    signal::SignalFrame,
    thingbuf::ThingBuf,
};
use spectrum_analyzer::FrequencySpectrum;

use crate::util::hash_str;

pub mod control;
pub mod entropy;
pub mod manual;

pub use control::Control;

/// Shared buffer for passing FFT spectrum data between the analyzer thread and consumers.
pub type SpectrumBuffer = Arc<ThingBuf<Arc<FrequencySpectrum>>>;

/// Window size used by the FFT analyzer (power-of-2, balances frequency resolution vs latency).
pub const FFT_WINDOW_SIZE: usize = 8192;

/// Stable node ID for [`ExcitorReader`].
const EXCITOR_READER_ID: u64 = hash_str(concat!(module_path!(), "::ExcitorReader"));

/// Wires a slice of [`Control`] shared values into the audio graph as flat audio outputs.
///
/// Layout: `[real_0, imag_0, real_1, imag_1, …, real_n, imag_n]`
///
/// - **Inputs:** 0
/// - **Outputs:** `controls.len() * 2`
///
/// The "writer" side (Manual UI / Mic / Random) lives entirely outside the graph and
/// sets control values via [`Control::real`] / [`Control::imaginary`] directly.
/// This node just forwards whatever is currently in those [`Shared`] cells as
/// audio-rate outputs on every tick.
#[derive(Clone)]
struct ExcitorReader {
    controls: Vec<Control>,
}

impl ExcitorReader {
    fn new(controls: Vec<Control>) -> Self {
        Self { controls }
    }
}

impl AudioUnit for ExcitorReader {
    fn inputs(&self) -> usize {
        0
    }

    fn outputs(&self) -> usize {
        self.controls.len() * 2
    }

    fn tick(&mut self, _input: &[f32], output: &mut [f32]) {
        for (i, c) in self.controls.iter().enumerate() {
            output[2 * i] = c.real.value();
            output[2 * i + 1] = c.imaginary.value();
        }
    }

    fn process(&mut self, size: usize, _input: &BufferRef, output: &mut BufferMut) {
        let n = self.outputs();
        // Control values are piecewise-constant; read once per block and splat.
        let mut out = vec![0.0_f32; n];
        self.tick(&[], &mut out);
        // `size` is a scalar sample count; `at_mut` takes a SIMD frame index (0..7).
        let simd_frames = size / 8;
        for ch in 0..n {
            let splat = wide::f32x8::splat(out[ch]);
            for s in 0..simd_frames {
                *output.at_mut(ch, s) = splat;
            }
            // Handle any remaining scalar samples (when size is not divisible by 8).
            for j in (simd_frames * 8)..size {
                output.set_f32(ch, j, out[ch]);
            }
        }
    }

    fn set_sample_rate(&mut self, _rate: f64) {}

    fn reset(&mut self) {}

    fn allocate(&mut self) {}

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        SignalFrame::new(self.outputs())
    }

    fn get_id(&self) -> u64 {
        EXCITOR_READER_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/// Push an [`ExcitorReader`] into `net` and return its [`NodeId`].
///
/// The caller is responsible for keeping the [`Control`] handles alive and
/// setting their values from the appropriate source (UI / mic / random).
/// The returned [`NodeId`] can be connected to downstream ADSR nodes with
/// `net.connect(excitor_id, 2*i, adsr_id, primary_input)` etc.
pub fn mount_excitor_au(net: &mut Net, controls: Vec<Control>) -> NodeId {
    net.push(Box::new(ExcitorReader::new(controls)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_controls(n: usize) -> Vec<Control> {
        (0..n).map(|_| Control::default()).collect()
    }

    #[test]
    fn outputs_equals_two_per_control() {
        let reader = ExcitorReader::new(make_controls(3));
        assert_eq!(reader.outputs(), 6);
    }

    #[test]
    fn inputs_is_zero() {
        let reader = ExcitorReader::new(make_controls(2));
        assert_eq!(reader.inputs(), 0);
    }

    #[test]
    fn tick_reflects_control_values() {
        let controls = make_controls(2);
        controls[0].real.set_value(0.5);
        controls[0].imaginary.set_value(0.25);
        controls[1].real.set_value(1.0);
        controls[1].imaginary.set_value(0.75);

        let mut reader = ExcitorReader::new(controls);
        let mut out = vec![0.0_f32; 4];
        reader.tick(&[], &mut out);

        assert!((out[0] - 0.5).abs() < 1e-6, "real_0 mismatch");
        assert!((out[1] - 0.25).abs() < 1e-6, "imag_0 mismatch");
        assert!((out[2] - 1.0).abs() < 1e-6, "real_1 mismatch");
        assert!((out[3] - 0.75).abs() < 1e-6, "imag_1 mismatch");
    }

    #[test]
    fn tick_zero_controls_no_panic() {
        let mut reader = ExcitorReader::new(vec![]);
        reader.tick(&[], &mut []);
    }

    #[test]
    fn mount_returns_node_id() {
        let mut net = Net::new(0, 1);
        let controls = make_controls(2);
        let id = mount_excitor_au(&mut net, controls);
        // NodeId should be valid (net can display without panic)
        let _ = net.display();
        let _ = id;
    }
}
