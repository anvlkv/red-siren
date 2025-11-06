//! Web (future) runtime stub.
//!
//! PURPOSE
//! -------
//! Placeholder implementation for an upcoming WebAudio / WASM backend.
//! For now it returns a `NullController`, allowing callers to enable the
//! `rt_web` feature without breaking builds.
//!
//! DESIGN (MAYA DRY KISS)
//! ----------------------
//! - Do not pre-implement speculative logic.
//! - Provide only the public surface required by the runtime facade
//!   (`crate::rt::make_stream_controller`).
//! - Keep comments explaining how to extend so future work has clear context.
//!
//! EXTENSION PLAN
//! --------------
//! When implementing the real web runtime, replace the `NullController` return
//! with a concrete type that:
//!   1. Manages a WebAudio graph (likely via wasm-bindgen / web-sys).
//!   2. Mirrors the behavior of the CPAL controller regarding:
//!        - start/stop/pause/resume
//!        - excitement source switching (Mic vs entropy)
//!        - layout/config graph rebuild (crossfade semantics may differ
//!          on WebAudio; emulate or approximate as needed).
//!        - output snoop snapshots (could be implemented by tapping nodes
//!          into ScriptProcessor / AudioWorklet buffers).
//!
//!   3. Handles microphone permission asynchronously (browser permission API)
//!      and surfaces errors via the same error types from `common`.
//!
//! FEATURE FLAG
//! ------------
//! Enabled with: `rt_web`
//! Precedence in `crate::rt::make_stream_controller`:
//!   - If `rt_cpal` is enabled, CPAL takes precedence.
//!   - Else if `rt_web` enabled, this module is used.
//!   - Else fallback to `NullController` directly.
//!
//! NOTE
//! ----
//! The current implementation intentionally returns a `NullController` so that
//! enabling `rt_web` does not silently change runtime behavior until the real
//! web backend is ready.
use crate::rt::{NullController, StreamController};

/// Factory for the (future) web runtime.
///
/// Currently returns a `NullController` as a placeholder.
pub fn make_stream_controller() -> common::error::Result<Box<dyn StreamController + Send + Sync>> {
    Ok(Box::new(NullController))
}

// (Optional) Future scaffold example:
//
// struct WebController { /* fields */ }
//
// impl StreamController for WebController {
//     fn start(
//         &self,
//         _layout: &common::instrument::Layout,
//         _config: &common::instrument::Config,
//         _source: crate::rt::ExcitementSource,
//     ) -> common::error::Result<()> {
//         // TODO: Initialize / connect WebAudio graph
//         Ok(())
//     }
//     fn stop(&self) -> common::error::Result<()> { Ok(()) }
//     fn pause(&self) -> common::error::Result<()> { Ok(()) }
//     fn resume(&self) -> common::error::Result<()> { Ok(()) }
//     fn on_excitement_source_changed(
//         &self,
//         _source: crate::rt::ExcitementSource,
//     ) -> common::error::Result<()> { Ok(()) }
//     fn on_layout_changed(
//         &self,
//         _layout: &common::instrument::Layout,
//         _config: &common::instrument::Config,
//     ) -> common::error::Result<()> { Ok(()) }
//     fn snapshot_output_snoop(&self, _group: usize, _key: usize) -> Vec<f32> { Vec::new() }
//     fn snapshot_all_output_snoops(&self) -> Vec<(u8,u8,Vec<f32>)> { Vec::new() }
// }
