//! Random activation stream.
//!
//! Generates pseudo-random activation values in [0.0, 1.0] on its own thread,
//! feeding them into a caller-provided producer closure (ring buffer writer).
//! These values are designed to be used directly by RandomActivator node,
//! bypassing FFT analysis.
//!
//! Control messages (Pause / Resume / Shutdown) are supported via the shared
//! `Control` enum and acknowledged with `ControlInvocationResult`.
//!
//! MAYA DRY KISS:
//! - Generates activation-ready values [0.0, 1.0] for direct use
//! - Smoother transitions with bias towards lower values
//! - Simple loop with consistent sample generation rate
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

/// Generate samples continuously at audio rate for smooth activation
/// Sleep interval in microseconds between samples (22 µs ≈ 44.1kHz)
pub const SAMPLE_INTERVAL_US: u64 = 22;

/// Spawn a thread that generates activation values in the range [0.0, 1.0]
/// at a consistent rate and feeds them into the provided producer.
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

            // Generate activation value in [0.0, 1.0] with bias towards lower values
            // Use multiple samples to create smoother distribution
            let r1 = rng.f32();
            let r2 = rng.f32();
            let r3 = rng.f32();

            // Average for centered distribution, then square for lower bias
            let avg = (r1 + r2 + r3) / 3.0;
            let value = avg * avg; // Bias towards lower values

            let buf = [value as f64];

            // Feed into producer; drop if buffer is full.
            let _ = produce_sample(&buf);

            // Sleep at consistent rate for smooth activation updates
            thread::sleep(Duration::from_micros(SAMPLE_INTERVAL_US));
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
