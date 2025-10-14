//! Random noise activation stream.
//!
//! Generates pseudo-random noise samples in [-1.0, 1.0] on its own thread,
//! feeding them into a caller-provided producer closure (ring buffer writer,
//! analyzer feeder, etc).
//!
//! Control messages (Pause / Resume / Shutdown) are supported via the shared
//! `Control` enum and acknowledged with `ControlInvocationResult`.
//!
//! MAYA DRY KISS:
//! - Minimal surface: only logic required to emulate an "activation source"
//!   when microphone input is not selected (entropy mode).
//! - No over-engineering: simple loop with randomized sleep interval.
//!
//! Extending:
//! - If future activation behaviors (e.g. shaped noise, envelopes) are needed,
//!   add parameters to the constructor or create additional stream modules.

use std::{
    sync::mpsc::{channel, Receiver, Sender},
    thread,
    time::Duration,
};

use common::error::InstrumentError;
use fastrand::Rng;

use super::{Control, ControlInvocationResult, ProdType};

/// Minimum delay between noise bursts in milliseconds.
pub const NOISE_DELAY_MIN_MS: u64 = 5;
/// Maximum delay between noise bursts in milliseconds.
pub const NOISE_DELAY_MAX_MS: u64 = 50;

/// Spawn a thread that generates random noise samples in the range [-1.0, 1.0]
/// with random delays between bursts and feeds them into the provided producer.
///
/// The thread supports pause, resume, and shutdown via `Control` messages.
///
/// Returns a `Sender<Control>` to control the noise generator and the `JoinHandle` of the spawned thread.
pub fn spawn_owned_noise_stream<FProd>(
    make_prod: FProd,
) -> Result<(Sender<Control>, thread::JoinHandle<()>), InstrumentError>
where
    FProd: Send + 'static + FnOnce() -> Box<ProdType>,
{
    let (tx, rx): (Sender<Control>, Receiver<Control>) = channel();
    // One-shot init channel so the caller learns whether thread init succeeded.
    let (init_tx, init_rx) = channel::<Result<(), InstrumentError>>();

    let handle = thread::spawn(move || {
        let mut rng = Rng::new();
        // Build producer inside the owner thread.
        let mut produce_sample = make_prod();

        // Signal successful initialization.
        let _ = init_tx.send(Ok(()));

        // Control state
        let mut paused = false;

        'outer: loop {
            if paused {
                // When paused, block waiting for resume or shutdown.
                match rx.recv() {
                    Ok(Control::Pause(ret)) => {
                        // Already paused; acknowledge.
                        let _ = ret.send(ControlInvocationResult::Ok(()));
                    }
                    Ok(Control::Resume(ret)) => {
                        paused = false;
                        let _ = ret.send(ControlInvocationResult::Ok(()));
                    }
                    Ok(Control::Shutdown(ret)) => {
                        let _ = ret.send(ControlInvocationResult::Ok(()));
                        break 'outer;
                    }
                    Err(_) => break 'outer,
                }
                continue;
            }

            // Drain any immediate control messages without blocking.
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Control::Pause(ret) => {
                        paused = true;
                        let _ = ret.send(ControlInvocationResult::Ok(()));
                    }
                    Control::Resume(ret) => {
                        // Already running; acknowledge.
                        let _ = ret.send(ControlInvocationResult::Ok(()));
                    }
                    Control::Shutdown(ret) => {
                        let _ = ret.send(ControlInvocationResult::Ok(()));
                        break 'outer;
                    }
                }
            }

            // Generate one noise sample in [-1.0, 1.0]
            let value = rng.f32() * 2.0 - 1.0;
            let buf = [value as f64];

            // Feed into producer; drop if buffer is full.
            let _ = produce_sample(&buf);

            // Sleep a random delay between configured bounds.
            let delay_ms = if NOISE_DELAY_MIN_MS >= NOISE_DELAY_MAX_MS {
                NOISE_DELAY_MIN_MS
            } else {
                rng.u64(NOISE_DELAY_MIN_MS..=NOISE_DELAY_MAX_MS)
            };
            thread::sleep(Duration::from_millis(delay_ms));
        }
    });

    // Wait for init result from the owner thread.
    match init_rx.recv() {
        Ok(Ok(())) => Ok((tx, handle)),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(InstrumentError::StartFailed {
            detail: Some("noise stream owner init channel closed".into()),
        }),
    }
}
