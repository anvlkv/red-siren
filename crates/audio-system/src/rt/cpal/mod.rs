//! CPAL runtime module.
//!
//! Structure:
//! - `engine.rs`: Implements `make_stream_controller()` returning a boxed
//!   `StreamController` concrete (the CPAL-backed controller).
//! - `stream/`: Spawn helpers & control protocol (input/output/noise).
//! - `mic_permission.rs`: Lightweight microphone availability / permission probe.
//! - `tuner.rs`: CPAL-based implementation of the generic `TunerRuntime`.
//!
//! Re-exports:
//! - `make_stream_controller()` consumed by `crate::rt::make_stream_controller`
//!   when the `rt_cpal` feature is enabled.
//! - `check_mic_permission()` for health / setup flows.
//! - `make_tuner_runtime()` used by the generic tuner facade.
//! - `stream` submodule (public) so higher layers can access spawn helpers.
//!
//! Design Notes (MAYA DRY KISS):
//! - Thin aggregation layer; heavy logic isolated in implementation files.
//! - No legacy aliases retained (old `cpal_audio` removed).
//! - Public surface is intentionally minimal & explicit.
mod engine;
mod mic_permission;
pub mod stream;
mod tuner;

pub use engine::make_stream_controller;
pub use mic_permission::check_mic_permission;
pub use tuner::make_tuner_runtime;

// Re-export commonly used control types for ergonomics.
// (Callers may still choose the longer `stream::` path.)
pub use stream::{Control, ControlInvocationResult};

/// Convenience prelude for typical CPAL runtime consumer needs.
/// Opt-in: callers explicitly import if they want shorter paths.
///
/// Example:
/// use audio_system::rt::cpal::prelude::*;
/// let controller = make_stream_controller().unwrap();
pub mod prelude {
    pub use super::stream::{
        spawn_owned_input_stream, spawn_owned_noise_stream, spawn_owned_output_stream, GenType,
        ProdType, STREAM_TIMEOUT_S,
    };
    pub use super::{
        check_mic_permission, make_stream_controller, make_tuner_runtime, Control,
        ControlInvocationResult,
    };
}
