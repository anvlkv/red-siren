use std::{
    collections::VecDeque,
    marker::PhantomData,
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::thingbuf::ThingBuf;
use parking_lot::Mutex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::util::hash_str;

const LPC_ID: u64 = hash_str(concat!(module_path!(), "::Lpc"));

pub const DEFAULT_BUFFER_SIZE: usize = 512;

pub const DEFAULT_FFT_SIZE: usize = DEFAULT_BUFFER_SIZE / 2 + 1;
// Sensible small LPC order for general audio. Can be tuned by creating another constructor.
pub const DEFAULT_LPC_ORDER: usize = 16;
pub const DEFAULT_LPC_ORDER_PLUS_ONE: usize = DEFAULT_LPC_ORDER + 1;

// Internal constants
const MIN_ENERGY: f32 = 1.0e-9;
// Tunables for confidence blending
const CONFIDENCE_C: f32 = 8.0; // Larger -> more conservative (less trust when error/r0 is high)
const ALPHA_MIN: f32 = 0.01; // Floor for alpha; 0.0 means can fully fall back to input
const ALPHA_MAX: f32 = 1.0; // Cap for alpha

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
pub struct Lpc<const B: usize, const R: usize, const P: usize, const PR: usize, const N: usize, O>
where
    O: Size<f32>,
{
    buffer: Arc<ThingBuf<f32>>,

    latest_data: Option<LpcData<P>>,

    // History of most recent P actual input samples.
    // Convention: history[0] is x[n], history[1] is x[n-1], ...
    history: [f32; P],

    // Sample rate info
    sample_rate: f64,

    //fft
    fft_planner: Arc<Mutex<RealFftPlanner<f32>>>,
    processing_handle: Arc<JoinHandle<()>>,
    processing_running: Arc<AtomicBool>,
    result_buffer: Arc<ThingBuf<LpcData<P>>>,

    _phantom: PhantomData<O>,
}

#[derive(Debug, Clone, Copy)]
struct LpcData<const P: usize> {
    /// AR coefficients a[0..P-1] for polynomial 1 + sum_i a[i] z^{-i}
    data: [f32; P],
    /// Final prediction error from Levinson–Durbin
    error: f32,
    /// r[0] (lag-0 autocorrelation / frame energy) used to normalize `error`
    r0: f32,
}

impl<const P: usize> Default for LpcData<P> {
    fn default() -> Self {
        Self {
            data: [0.0; P],
            error: 1.0,
            r0: 1.0,
        }
    }
}

impl<const B: usize, const R: usize, const P: usize, const PR: usize, const N: usize, O>
    Lpc<B, R, P, PR, N, O>
where
    O: Size<f32>,
{
    pub fn new() -> Self {
        debug_assert!(B.is_power_of_two(), "B must be a power of two");
        debug_assert!(P < B, "LPC order P must satisfy P < B");
        debug_assert_eq!(R, B / 2 + 1, "R must be B/2 + 1");
        debug_assert!(PR == P + 1, "PR must be P + 1");

        let sample_rate = DEFAULT_SR;
        let mut fft_planner = RealFftPlanner::<f32>::new();
        let real = fft_planner.plan_fft_forward(B);
        let inverse = fft_planner.plan_fft_inverse(B);

        let buffer = Arc::new(ThingBuf::new(B + B / 2));
        let result_buffer = Arc::new(ThingBuf::new(2));

        let (processing_handle, processing_running) = Self::start_processing(
            sample_rate,
            buffer.clone(),
            real,
            inverse,
            result_buffer.clone(),
        );

        Self {
            buffer,
            latest_data: None,
            history: [0.0; P],

            sample_rate,

            fft_planner: Arc::new(Mutex::new(fft_planner)),
            result_buffer,
            processing_handle,
            processing_running,

            _phantom: PhantomData,
        }
    }

    fn start_processing(
        sample_rate: f64,
        buffer: Arc<ThingBuf<f32>>,
        real: Arc<dyn RealToComplex<f32>>,
        inverse: Arc<dyn ComplexToReal<f32>>,
        result_buffer: Arc<ThingBuf<LpcData<P>>>,
    ) -> (Arc<JoinHandle<()>>, Arc<AtomicBool>) {
        let running = Arc::new(AtomicBool::new(true));
        let running_thread = running.clone();
        let join = spawn(move || {
            log::info!("LPC thread started");
            let mut prev_window: VecDeque<f32> = VecDeque::with_capacity(B / 2);
            loop {
                if !running_thread.load(Ordering::SeqCst) {
                    log::info!("LPC thread stopped");
                    break;
                }
                // analyze every B / 2 samples
                if let Some(remaining_len) = B
                    .checked_sub(buffer.len() + prev_window.len())
                    .filter(|s| *s != 0)
                {
                    sleep(Duration::from_secs_f64(remaining_len as f64 / sample_rate));
                } else {
                    let window: [f32; B] = core::array::from_fn(|_| {
                        prev_window
                            .pop_front()
                            .or_else(|| buffer.pop())
                            .unwrap_or_default()
                    });
                    let r = Self::autocorrelation_first_p(&window, &real, &inverse);
                    let r0 = r[0];
                    let (data, error) = Self::levinson_durbin(&r);

                    match result_buffer.push(LpcData { data, error, r0 }) {
                        Ok(_) => {}
                        Err(back) => {
                            _ = result_buffer
                                .pop()
                                .and_then(|_| result_buffer.push(back.into_inner()).ok());
                        }
                    }
                    prev_window = VecDeque::from_iter(window[B / 2..].iter().copied());
                }
            }
        });

        (Arc::new(join), running)
    }

    // Compute first P+1 lags of circular autocorrelation using FFT.
    // Returns r[0..=P].
    fn autocorrelation_first_p(
        signal: &[f32; B],
        real: &Arc<dyn RealToComplex<f32>>,
        inverse: &Arc<dyn ComplexToReal<f32>>,
    ) -> [f32; PR] {
        // 1) Real FFT of length B -> R = B/2 + 1 complex bins using realfft planner.
        // Copy signal to a mutable local buffer for realfft.
        let mut inbuf = [0.0f32; B];
        inbuf.copy_from_slice(signal);
        let mut spectrum = [Complex32::default(); R];
        // Forward real-to-complex FFT
        real.process(&mut inbuf, &mut spectrum)
            .expect("realfft forward");

        // 2) Power spectrum: |X[k]|^2 in half-spectrum format expected by realfft inverse.
        for bin in spectrum.iter_mut() {
            let mag2 = bin.norm_sqr();
            *bin = Complex32::new(mag2, 0.0);
        }

        // 3) Inverse FFT (complex-to-real) to get circular autocorrelation in time domain.
        let mut time = [0.0f32; B];
        inverse
            .process(&mut spectrum, &mut time)
            .expect("realfft inverse");

        // 4) Normalize by B (FFT and iFFT are unnormalized in realfft).
        let scale = 1.0 / (B as f32);
        let mut r = [0.0f32; PR];
        for i in 0..=P {
            r[i] = time[i] * scale;
        }
        // Stabilize r[0] to avoid zero/negative energies.
        if r[0] < MIN_ENERGY {
            r[0] = MIN_ENERGY;
        }
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

    fn buffer_sample_for_analysis(&mut self, x_n: f32) -> bool {
        let x_n = if x_n.is_normal() { x_n } else { 0.0 };

        // Write into circular frame buffer.
        let consumed = match self.buffer.push(x_n) {
            Ok(_) => true,
            Err(_) => {
                // Handle buffer overflow error
                _ = self.buffer.pop();
                _ = self.buffer.push(x_n);
                log::trace!("Buffer overflow in LPC analysis, discarding oldest sample");
                false
            }
        };

        // Push into history: shift right, put x_n at front.
        // This is O(P), but P is small by design.
        self.history.rotate_right(1);
        self.history[0] = x_n;

        if let Some(data) = self.result_buffer.pop() {
            self.latest_data = Some(data)
        }

        consumed
    }

    #[inline]
    fn confidence_alpha(&self) -> f32 {
        if let Some(LpcData { error, r0, .. }) = self.latest_data {
            // Normalize and guard against degenerate values
            if r0.is_finite() && r0 > 0.0 && error.is_finite() && error >= 0.0 {
                let ratio = (error / r0).clamp(0.0, 1.0e6);
                let alpha = 1.0 / (1.0 + CONFIDENCE_C * ratio);
                alpha.clamp(ALPHA_MIN, ALPHA_MAX)
            } else {
                ALPHA_MIN
            }
        } else {
            // No coefficients yet -> don’t trust predictor
            ALPHA_MIN
        }
    }

    fn predict_next_frame(&mut self, x_n: f32) -> Frame<f32, O> {
        let x_n = if x_n.is_normal() { x_n } else { 0.0 };

        // N-step-ahead prediction using current coefficients.
        // Seed with actual history (x[n], x[n-1], ..., x[n-P+1]).
        let mut predicted_frame = Frame::from_iter(self.history[..O::to_usize()].iter().copied());

        let alpha = self.confidence_alpha();
        if let Some(LpcData { data, .. }) = self.latest_data {
            let mut state = self.history;
            let frames = O::to_usize();
            for i in 0..frames {
                let mut y_hat = 0.0f32;
                for _ in 0..N / (frames - i) {
                    // Predict 1-step ahead from state (which currently has latest at state[0]).
                    let mut acc = 0.0f32;
                    for (ai, xi) in data.iter().zip(state.iter()) {
                        acc -= *ai * *xi;
                    }
                    y_hat = acc;

                    // Roll state: insert predicted at front for multi-step prediction.
                    state.rotate_right(1);
                    state[0] = y_hat;
                }
                predicted_frame[i] = alpha * y_hat + (1.0 - alpha) * x_n;
            }
        }

        predicted_frame
    }
}

impl<const B: usize, const R: usize, const P: usize, const PR: usize, const N: usize, O> AudioNode
    for Lpc<B, R, P, PR, N, O>
where
    O: Size<f32>,
{
    // Mix in generics to the ID to avoid collisions.
    const ID: u64 =
        LPC_ID + (B as u64) + ((R as u64) << 10) + ((P as u64) << 20) + ((N as u64) << 40);

    type Inputs = U1;
    type Outputs = O;

    fn reset(&mut self) {
        self.processing_running.store(false, Ordering::SeqCst);

        while !self.buffer.is_empty() {
            _ = self.buffer.pop();
        }

        while !self.result_buffer.is_empty() {
            _ = self.result_buffer.pop();
        }

        self.history.fill(0.0);

        let (real, inverse) = {
            let mut planer = self.fft_planner.lock();
            (planer.plan_fft_forward(B), planer.plan_fft_inverse(B))
        };

        let (processing_handle, processing_running) = Self::start_processing(
            self.sample_rate,
            self.buffer.clone(),
            real,
            inverse,
            self.result_buffer.clone(),
        );

        let old_handle = mem::replace(&mut self.processing_handle, processing_handle);
        if let Some(handle) = Arc::into_inner(old_handle) {
            handle.join().unwrap();
            log::debug!("LPC thread stop completed (join)");
        }

        self.processing_running = processing_running;
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let x_n = input[0];
        self.buffer_sample_for_analysis(x_n);
        self.predict_next_frame(x_n)
    }
}

/// Create LPC predictor node with default frame size and LPC order,
/// returning N-step-ahead prediction.
pub fn lpc<const N: usize>() -> An<
    Lpc<
        DEFAULT_BUFFER_SIZE,
        DEFAULT_FFT_SIZE,
        DEFAULT_LPC_ORDER,
        DEFAULT_LPC_ORDER_PLUS_ONE,
        N,
        U1,
    >,
> {
    An(Lpc::<
        DEFAULT_BUFFER_SIZE,
        DEFAULT_FFT_SIZE,
        DEFAULT_LPC_ORDER,
        DEFAULT_LPC_ORDER_PLUS_ONE,
        N,
        U1,
    >::new())
}

/// Create LPC predictor bank with default frame size and LPC order,
/// returning N-step-ahead prediction, spread over O-frames
pub fn lpc_bank<const N: usize, O>() -> An<
    Lpc<DEFAULT_BUFFER_SIZE, DEFAULT_FFT_SIZE, DEFAULT_LPC_ORDER, DEFAULT_LPC_ORDER_PLUS_ONE, N, O>,
>
where
    O: Size<f32>,
{
    An(Lpc::<
        DEFAULT_BUFFER_SIZE,
        DEFAULT_FFT_SIZE,
        DEFAULT_LPC_ORDER,
        DEFAULT_LPC_ORDER_PLUS_ONE,
        N,
        O,
    >::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn test_lpc_1() {
        let node =
            constant(440.0) >> sine_phase::<f32>(0.1) >> split::<U2>() >> (lpc::<1>() | pass());

        assert_audio_unit_snapshot!(node);
    }
    #[test]
    fn test_lpc_2() {
        let node =
            constant(440.0) >> sine_phase::<f32>(0.1) >> split::<U2>() >> (lpc::<2>() | pass());

        assert_audio_unit_snapshot!(node);
    }
    #[test]
    fn test_lpc_7() {
        let node =
            constant(440.0) >> sine_phase::<f32>(0.1) >> split::<U2>() >> (lpc::<7>() | pass());

        assert_audio_unit_snapshot!(node);
    }

    #[test]
    fn test_lpc_bank_9() {
        let node = constant(440.0) >> sine_phase::<f32>(0.1) >> (lpc_bank::<9, U3>());

        assert_audio_unit_snapshot!(node);
    }
}
