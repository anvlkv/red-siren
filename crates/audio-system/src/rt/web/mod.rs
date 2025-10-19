//! Web runtime implementation for WASM compilation.
//!
//! PURPOSE
//! -------
//! This module provides WASM-specific functionality for audio processing
//! in Web Audio API + AudioWorklet environments. It no longer provides
//! the StreamController interface - that has moved to src-tauri layer.
//!
//! ARCHITECTURE CHANGE
//! -------------------
//! Previous: This module provided make_stream_controller() for Tauri integration
//! Current: This module only exports WASM runtime functions for AudioWorklet
//!
//! The StreamController implementation now lives in:
//! src-tauri/src/audio_worklet/controller.rs as WebAudioController
//!
//! MAYA DRY KISS
//! -------------
//! - Single responsibility: WASM runtime functions only
//! - No Tauri dependencies or knowledge
//! - Clean separation of concerns

#[cfg(feature = "rt_web_audio_unit")]
pub mod wasm_runtime;

#[cfg(feature = "rt_web_audio_unit")]
pub use wasm_runtime::*;

// The make_stream_controller function has been removed.
// StreamController implementation is now in src-tauri layer.
//
// This keeps the audio-system crate focused on pure DSP processing
// while Tauri integration happens in the appropriate layer.
