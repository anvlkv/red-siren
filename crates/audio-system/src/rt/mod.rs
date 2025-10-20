#[cfg(feature = "rt_cpal")]
pub mod cpal;

#[cfg(feature = "rt_web")]
pub mod web;

use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use common::tuner::Config as TunerConfig;

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
        tuner_config: &TunerConfig,
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
        tuner_config: &TunerConfig
    ) -> common::error::Result<()>;

    // Data taps
    fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32>;
    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)>;
    fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32>;
    fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)>;

    // Band control
    fn set_band_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()>;

    fn get_band_control(&self, key: common::NodeKey) -> common::error::Result<f32>;
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
        _tuner_config: &TunerConfig
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

    fn set_band_control(&self, _key: common::NodeKey, _value: f32) -> common::error::Result<()> {
        Ok(())
    }

    fn get_band_control(&self, _key: common::NodeKey) -> common::error::Result<f32> {
        Ok(0.0)
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
