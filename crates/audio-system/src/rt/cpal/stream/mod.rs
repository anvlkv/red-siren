//! CPAL stream control primitives.
//!
//! Extracted from previous backend implementation and centralized here so
//! higher-level crates only depend on `audio_system::rt` abstractions.
//!
//! Responsibilities:
//! - Defines control messages (`Control`) sent to owning audio threads.
//! - Defines `ControlInvocationResult` used for per-request acknowledgements.
//! - Exposes common timeout constant (`STREAM_TIMEOUT_S`).
//!
//! Implementation details (MAYA DRY KISS):
//! - Minimal surface: only what existing CPAL runtime needs.
//! - The concrete spawn helpers (input/output/random) live in sibling
//!   modules and are re-exported for convenience.
//!
//! Threading model summary:
//! - Each audio stream (input/output) or noise generator owns its thread.
//! - Control channel (mpsc) provides Pause/Resume/Shutdown with ack.
//! - A Shutdown message is expected to drop the underlying CPAL stream
//!   (or noise loop) and then acknowledge.
//!
//! Extending:
//! - Add new control variants only when strictly required by multiple
//!   runtime components.
//! - Keep this file lean; heavy logic stays in engine-level code.
use std::sync::mpsc::Sender;

pub mod input;
pub mod output;
pub mod playback;

pub use input::*;
pub use output::*;
pub use playback::*;

/// Maximum seconds CPAL waits inside build/playback callbacks before timing out.
pub const STREAM_TIMEOUT_S: u64 = 10;

/// Result of a control invocation, sent back to the caller per request.
pub type ControlInvocationResult = Result<(), String>;

/// Control messages for the stream owner thread.
pub enum Control {
    /// Pause stream processing (acknowledged via provided sender).
    Pause(Sender<ControlInvocationResult>),
    /// Resume stream processing (acknowledged via provided sender).
    Resume(Sender<ControlInvocationResult>),
    /// Terminate the stream / worker. Receiver should drop resources,
    /// then acknowledge and exit its loop.
    Shutdown(Sender<ControlInvocationResult>),
}
