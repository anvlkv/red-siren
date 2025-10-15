//! Runtime abstraction layer for audio backends.
//!
//! WHY:
//! - Centralizes selection of a concrete audio runtime (CPAL now, Web later).
//! - Provides a stable trait `StreamController` consumed by higher-level
//!   instrument & tuner engines (in the backend crate).
//! - Enables building the application without any native audio backend
//!   (feature flags control inclusion).
//!
//! EXTENDING:
//! - Add a new submodule under `rt/` (e.g. `web`) implementing
//!   `make_stream_controller()` that returns a boxed implementor of
//!   `StreamController`.
//! - Gate it behind a Cargo feature (e.g. `rt_web`) and update the factory
//!   precedence logic below if multiple runtimes may be enabled together.
//!
//! PRINCIPLES (MAYA DRY KISS):
//! - Minimal surface: only what the backend needs today.
//! - No legacy compatibility aliases retained (old `cpal_audio` removed).
//! - Keep implementation details (threads, buffers, device specifics) inside
//!   concrete runtime modules (`cpal`, future `web`).
//!
//! FEATURE FLAGS:
//! - `rt_cpal`: enables native CPAL runtime (desktop).
//! - `rt_web`: placeholder for future WebAudio / WASM implementation.
//! - If none enabled, a `NullController` is used (no-op, silent).

#[cfg(feature = "rt_cpal")]
pub mod cpal;

#[cfg(feature = "rt_web")]
pub mod web;

use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};

/// Source of activation energy driving instrument strings / nodes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ActivationSource {
    /// Pseudo-random / noise entropy (internal generator).
    #[default]
    Entropy,
    /// Live microphone input (if permission & capture available).
    Mic,
}

impl From<u8> for ActivationSource {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Entropy,
            _ => Self::Mic,
        }
    }
}

impl From<ActivationSource> for u8 {
    fn from(v: ActivationSource) -> Self {
        match v {
            ActivationSource::Entropy => 0,
            ActivationSource::Mic => 1,
        }
    }
}

/// Unified interface the higher-level engine uses.
///
/// Implementations manage:
/// - Graph construction / re-construction on layout or config changes.
/// - Audio I/O lifecycle (start/stop/pause/resume).
/// - Activation source switching (mic vs entropy).
/// - Data snoops (per-string sample snapshots).
pub trait StreamController {
    // Lifecycle
    fn start(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        source: ActivationSource,
    ) -> common::error::Result<()>;
    fn stop(&self) -> common::error::Result<()>;
    fn pause(&self) -> common::error::Result<()>;
    fn resume(&self) -> common::error::Result<()>;

    // Reactivity
    fn on_activation_source_changed(&self, source: ActivationSource) -> common::error::Result<()>;
    fn on_layout_changed(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
    ) -> common::error::Result<()>;

    // Data taps
    fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32>;
    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)>;
    fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32>;
    fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)>;
    }

/// Null / no-op runtime used when no concrete backend feature is enabled.
///
/// Provides graceful degradation:
/// - Calls succeed so UI logic stays consistent.
/// - Data taps return empty vectors.
/// - No threads or devices are opened.
#[derive(Debug, Default)]
pub struct NullController;

impl StreamController for NullController {
    fn start(
        &self,
        _layout: &InstrumentLayout,
        _config: &InstrumentConfig,
        _source: ActivationSource,
    ) -> common::error::Result<()> {
        Ok(())
    }

    fn stop(&self) -> common::error::Result<()> {
        Ok(())
    }

    fn pause(&self) -> common::error::Result<()> {
        Ok(())
    }

    fn resume(&self) -> common::error::Result<()> {
        Ok(())
    }

    fn on_activation_source_changed(&self, _source: ActivationSource) -> common::error::Result<()> {
        Ok(())
    }

    fn on_layout_changed(
        &self,
        _layout: &InstrumentLayout,
        _config: &InstrumentConfig,
    ) -> common::error::Result<()> {
        Ok(())
    }

    fn snapshot_output_snoop(&self, _group: usize, _key: usize) -> Vec<f32> {
        Vec::new()
    }

    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        Vec::new()
    }

    fn snapshot_activation_snoop(&self, _group: usize, _key: usize) -> Vec<f32> {
        Vec::new()
    }

    fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        Vec::new()
    }
}

/// Factory returning the highest-precedence available runtime.
///
/// Precedence (current):
/// 1. rt_cpal
/// 2. rt_web
/// 3. NullController (fallback)
pub fn make_stream_controller() -> common::error::Result<Box<dyn StreamController + Send + Sync>> {
    #[cfg(feature = "rt_cpal")]
    {
        return cpal::make_stream_controller();
    }
    #[cfg(all(not(feature = "rt_cpal"), feature = "rt_web"))]
    {
        return web::make_stream_controller();
    }
    Ok(Box::new(NullController::default()))
}
/// Indicates whether the active build includes a microphone-capable backend.
pub fn supports_mic() -> bool {
    #[cfg(feature = "rt_cpal")]
    {
        return true;
    }
    #[cfg(not(feature = "rt_cpal"))]
    {
        return false;
    }
}
/// Mic permission / availability check.
/// Returns Ok(()) on success; in non-mic builds returns an error so callers
/// can mark permission as false.
pub async fn check_mic_permission() -> Result<(), common::error::HealthError> {
    #[cfg(feature = "rt_cpal")]
    {
        return cpal::check_mic_permission().await;
    }
    #[cfg(not(feature = "rt_cpal"))]
    {
        return Err(common::error::HealthError::MicPermissionCheckFailed {
            detail: Some("no_mic_runtime".into()),
        });
    }
}
/// Spectrum / tuner runtime abstraction.
///
/// Backend polls `poll_spectrum` periodically (e.g. every 20ms). Implementations
/// may accumulate samples internally until a full frame (FFT) is ready.
pub trait TunerRuntime: Send + Sync {
    fn start(&self, config: &common::tuner::Config) -> common::error::Result<()>;
    fn stop(&self);
    fn update_config(&self, config: &common::tuner::Config);
    /// Poll for latest spectrum data (non-blocking). Returns None if not ready.
    fn poll_spectrum(&self) -> Option<common::tuner::SpectrumData>;
}
/// Null (no-op) tuner runtime.
#[derive(Default)]
struct NullTunerRuntime;
impl TunerRuntime for NullTunerRuntime {
    fn start(&self, _config: &common::tuner::Config) -> common::error::Result<()> {
        Ok(())
    }
    fn stop(&self) {}
    fn update_config(&self, _config: &common::tuner::Config) {}
    fn poll_spectrum(&self) -> Option<common::tuner::SpectrumData> {
        None
    }
}
/// Factory producing a tuner runtime (CPAL, web, or null).
pub fn make_tuner_runtime() -> Box<dyn TunerRuntime + Send + Sync> {
    #[cfg(feature = "rt_cpal")]
    {
        if let Some(r) = cpal::make_tuner_runtime() {
            return r;
        }
    }
    #[cfg(all(not(feature = "rt_cpal"), feature = "rt_web"))]
    {
        if let Some(r) = web::make_tuner_runtime() {
            return r;
        }
    }
    Box::new(NullTunerRuntime::default())
}
