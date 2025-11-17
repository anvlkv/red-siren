use fundsp::fft::{inverse_fft, real_fft};
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const LPC_ID: u64 = hash_str(concat!(module_path!(), "::Lpc"));

pub const DEFAULT_BUFFER_SIZE: usize = 512;

pub const DEFAULT_FFT_SIZE: usize = DEFAULT_BUFFER_SIZE / 2 + 1;
// Sensible small LPC order for general audio. Can be tuned by creating another constructor.
pub const DEFAULT_LPC_ORDER: usize = 16;
pub const DEFAULT_LPC_ORDER_PLUS_ONE: usize = DEFAULT_LPC_ORDER + 1;

// Internal constants
const MIN_ENERGY: f32 = 1.0e-9;
const HOP_DIVISOR: usize = 2; // analyze every B / 2 samples

/// LPC predictor with N-step-ahead prediction.
///
/// Design:
/// - Maintains an analysis frame of size B (power of two).
/// - Computes circular autocorrelation via FFT (Wiener–Khinchin).
/// - Solves for AR coefficients of order P with Levinson–Durbin.
/// - Predicts x[n+N] using AR model, seeding from current history up to x[n].
///
/// Generics:
/// - `B` = analysis frame size (power of 2)
/// - `R` = must be B/2 + 1
/// - `P` = LPC order (small, e.g., 8–24 for speech/music; must satisfy P + 1 <= B)
/// - `PR` = must be P + 1
/// - `N` = steps ahead to predict
#[derive(Clone)]
pub struct Lpc<const B: usize, const R: usize, const P: usize, const PR: usize, const N: usize> {
    // Analysis frame (circular). Rotation does not affect power spectrum magnitude,
    // so we can pass this directly to FFT without reordering.
    buffer: [f32; B],
    write_pos: usize,
    // Whether we have filled at least one full frame.
    frame_filled: bool,
    // Hop scheduling (analyze every hop samples).
    hop: usize,
    hop_counter: usize,

    // AR coefficients a[0..P-1] for polynomial 1 + sum_i a[i] z^{-i}
    a: [f32; P],
    pred_error: f32,

    // History of most recent P actual input samples.
    // Convention: history[0] is x[n], history[1] is x[n-1], ...
    history: [f32; P],

    // Sample rate info (kept for trait completeness)
    sample_duration: f32,
    sample_rate: f64,
}

impl<const B: usize, const R: usize, const P: usize, const PR: usize, const N: usize>
    Lpc<B, R, P, PR, N>
{
    pub fn new() -> Self {
        debug_assert!(B.is_power_of_two(), "B must be a power of two");
        debug_assert!(P < B, "LPC order P must satisfy P < B");
        debug_assert_eq!(R, B / 2 + 1, "R must be B/2 + 1");
        debug_assert!(PR == P + 1, "PR must be P + 1");

        let sample_rate = DEFAULT_SR;
        let sample_duration = convert(1.0 / sample_rate);

        Self {
            buffer: [0.0; B],
            write_pos: 0,
            frame_filled: false,
            hop: B / HOP_DIVISOR,
            hop_counter: 0,

            a: [0.0; P],
            pred_error: 1.0,

            history: [0.0; P],

            sample_duration,
            sample_rate,
        }
    }

    // Compute first P+1 lags of circular autocorrelation using FFT.
    // Returns r[0..=P].
    fn autocorrelation_first_p(signal: &[f32; B]) -> [f32; PR] {
        // 1) Real FFT of length B -> R = B/2 + 1 complex bins.
        let mut x_spec = [Complex32::default(); R];
        real_fft(signal, &mut x_spec);

        // 2) Build full power spectrum |X[k]|^2 with Hermitian symmetry for IFFT of length B.
        let mut power_spectrum = [Complex32::default(); B];
        let n = B;
        // DC and Nyquist
        power_spectrum[0] = Complex32::new(x_spec[0].norm_sqr(), 0.0);
        power_spectrum[n / 2] = Complex32::new(x_spec[n / 2].norm_sqr(), 0.0);
        // Positive frequencies and mirrored negatives
        for k in 1..(n / 2) {
            let v = x_spec[k].norm_sqr();
            let c = Complex32::new(v, 0.0);
            power_spectrum[k] = c;
            power_spectrum[n - k] = c;
        }

        log::debug!("compute inverse fft of: {:?}", power_spectrum);
        // 3) Inverse FFT to get circular autocorrelation in time domain.
        let mut time = [Complex32::default(); B];
        inverse_fft(&power_spectrum, &mut time);

        // 4) Normalize by B (inverse scaling depends on FFT impl; we take the safe route).
        log::debug!("scaling...");
        let scale = 1.0 / (B as f32);
        let mut r = [0.0f32; PR];
        for i in 0..=P {
            r[i] = time[i].re * scale;
        }
        // Stabilize r[0] to avoid zero/negative energies.
        if r[0] < MIN_ENERGY {
            r[0] = MIN_ENERGY;
        }
        log::debug!("done: {r:?}");
        r
    }

    // Levinson–Durbin recursion to solve for AR coefficients.
    // Input: r[0..=P]
    // Output: (a[0..P-1], final prediction error)
    fn levinson_durbin(r: &[f32]) -> ([f32; P], f32) {
        let mut a = [0.0f32; P];
        let mut e = r[0];

        if e <= MIN_ENERGY || !e.is_finite() {
            return (a, MIN_ENERGY);
        }

        for i in 0..P {
            // Compute reflection coefficient k.
            // Note the minus sign to match AR polynomial 1 + sum a[i] z^{-i}.
            let mut acc = r[i + 1];
            for j in 0..i {
                acc += a[j] * r[i - j];
            }
            let mut k = -acc / e;

            // Clamp k for stability.
            k = k.clamp(-0.999_9, 0.999_9);

            // Update AR coefficients (a_new[0..=i]).
            let mut a_new_i = [0.0f32; P];
            a_new_i[i] = k;
            for j in 0..i {
                a_new_i[j] = a[j] + k * a[i - 1 - j];
            }
            // Copy back only up to i
            a[..=i].copy_from_slice(&a_new_i[..=i]);

            // Update prediction error.
            e *= 1.0 - k * k;
            if e < MIN_ENERGY || !e.is_finite() {
                e = MIN_ENERGY;
                break;
            }
        }

        (a, e.max(MIN_ENERGY))
    }

    // Update analysis (if due) and history with the new input sample,
    // then return N-step-ahead prediction.
    fn tick_internal(&mut self, x_n: f32) -> f32 {
        // Write into circular frame buffer.
        self.buffer[self.write_pos] = x_n;
        self.write_pos = (self.write_pos + 1) % B;

        // Mark frame filled after first full cycle.
        if !self.frame_filled && self.write_pos == 0 {
            self.frame_filled = true;
        }

        // Push into history: shift right, put x_n at front.
        // This is O(P), but P is small by design.
        self.history.rotate_right(1);
        self.history[0] = x_n;

        // Recompute LPC coefficients every hop once the frame is filled.
        self.hop_counter += 1;
        if self.frame_filled && self.hop_counter >= self.hop {
            self.hop_counter = 0;

            let r = Self::autocorrelation_first_p(&self.buffer);
            let (a, e) = Self::levinson_durbin(&r);

            self.a = a;
            self.pred_error = e;
        }

        // N-step-ahead prediction using current coefficients.
        // Seed with actual history (x[n], x[n-1], ..., x[n-P+1]).
        let mut state = self.history;
        let mut y_hat = 0.0f32;
        let steps = if N > 0 { N } else { 1 };
        for _ in 0..steps {
            // Predict 1-step ahead from state (which currently has latest at state[0]).
            let mut acc = 0.0f32;
            for (ai, xi) in self.a.iter().zip(state.iter()) {
                acc -= *ai * *xi;
            }
            y_hat = acc;

            // Roll state: insert predicted at front for multi-step prediction.
            state.rotate_right(1);
            state[0] = y_hat;
        }

        y_hat
    }
}

impl<const B: usize, const R: usize, const P: usize, const PR: usize, const N: usize> AudioNode
    for Lpc<B, R, P, PR, N>
{
    // Mix in generics to the ID to avoid collisions.
    const ID: u64 =
        LPC_ID + (B as u64) + ((R as u64) << 10) + ((P as u64) << 20) + ((N as u64) << 40);

    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.frame_filled = false;
        self.hop_counter = 0;

        self.a.fill(0.0);
        self.pred_error = 1.0;

        self.history.fill(0.0);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.sample_duration = convert(1.0 / sample_rate);
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let output = self.tick_internal(input[0]);
        [output].into()
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        // Process SIMD blocks
        for i in 0..full_simd_items(size) {
            let element: [f32; SIMD_N] = core::array::from_fn(|j| {
                let input_sample = input.at_f32(0, (i << SIMD_S) + j);
                self.tick_internal(input_sample)
            });
            output.set(0, i, F32x::new(element));
        }

        // Process remainder with the default helper.
        self.process_remainder(size, input, output);
    }
}

/// Create LPC predictor node with default frame size and LPC order,
/// returning N-step-ahead prediction.
pub fn lpc<const N: usize>(
) -> An<Lpc<DEFAULT_BUFFER_SIZE, DEFAULT_FFT_SIZE, DEFAULT_LPC_ORDER, DEFAULT_LPC_ORDER_PLUS_ONE, N>>
{
    An(Lpc::<
        DEFAULT_BUFFER_SIZE,
        DEFAULT_FFT_SIZE,
        DEFAULT_LPC_ORDER,
        DEFAULT_LPC_ORDER_PLUS_ONE,
        N,
    >::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_lpc_1() {
        let node = sine_hz::<f32>(440.0) >> split::<U2>() >> (lpc::<1>() | pass());

        assert_audio_unit_snapshot!(node);
    }
    #[test]
    fn test_lpc_2() {
        let node = sine_hz::<f32>(440.0) >> split::<U2>() >> (lpc::<2>() | pass());

        assert_audio_unit_snapshot!(node);
    }
    #[test]
    fn test_lpc_7() {
        let node = sine_hz::<f32>(440.0) >> split::<U2>() >> (lpc::<7>() | pass());

        assert_audio_unit_snapshot!(node);
    }
}
