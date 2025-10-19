#[cfg(any(feature = "rt_web", feature = "rt_web_audio_unit"))]
pub mod web;

// Re-export audio types from common
pub use common::audio::{ActivationSource, StreamController, TunerRuntime};

use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use common::tuner::Config as TunerConfig;

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
        _tuner_config: &TunerConfig,
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
        _tuner_config: &TunerConfig,
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

    fn set_band_control(&self, _group: u8, _key: u8, _value: f32) -> common::error::Result<()> {
        Ok(())
    }
}

// NOTE: make_stream_controller() has been removed from this crate.
//
// StreamController implementation now lives in the integration layer:
// - Web Audio: Implemented in src-tauri/src/audio_worklet/controller.rs
//
// This separation follows MAYA DRY KISS principles:
// - audio-system focuses purely on DSP processing
// - Integration concerns handled in appropriate layers
// NOTE: supports_mic() has been removed from this crate.
//
// Microphone capability checks now happen in the integration layer:
// - src-tauri handles browser permission APIs for web runtime
// - CPAL runtime retains its own mic support logic
//
// This keeps audio-system focused on pure DSP concerns.
// NOTE: check_mic_permission() has been removed from this crate.
//
// Permission checks are now handled in the integration layer:
// - src-tauri/src/audio_worklet/commands.rs for web runtime
// - CPAL runtime retains its own permission checking
//
// This separation allows proper async handling in the Tauri context.
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
    #[cfg(feature = "rt_web")]
    {
        if let Some(r) = web::make_tuner_runtime() {
            return r;
        }
    }
    Box::new(NullTunerRuntime::default())
}
