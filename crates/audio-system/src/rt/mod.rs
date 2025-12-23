#[cfg(feature = "rt_cpal")]
pub mod cpal;

#[cfg(feature = "rt_web")]
pub mod web;

use std::collections::BTreeMap;

use common::NodeKey;
use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use common::tuner::Config as TunerConfig;

pub type ProcessedOutputSpectrumSnapshot = (BTreeMap<u32, f32>, BTreeMap<u32, f32>);

/// Source of excitement energy driving instrument strings / nodes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ExcitementSource {
    /// Pseudo-random / noise entropy (internal generator).
    #[default]
    Entropy,
    /// Live microphone input (if permission & capture available).
    Mic,
}

impl From<u8> for ExcitementSource {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Entropy,
            _ => Self::Mic,
        }
    }
}

impl From<ExcitementSource> for u8 {
    fn from(v: ExcitementSource) -> Self {
        match v {
            ExcitementSource::Entropy => 0,
            ExcitementSource::Mic => 1,
        }
    }
}

/// Unified interface the higher-level engine uses.
///
/// Implementations manage:
/// - Graph construction / re-construction on layout or config changes.
/// - Audio I/O lifecycle (start/stop/pause/resume).
/// - Excitement source switching (mic vs entropy).
/// - Data snoops (per-string sample snapshots).
pub trait AudioRuntime {
    // Lifecycle
    fn start(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        source: ExcitementSource,
        tuner_config: &TunerConfig,
    ) -> common::error::Result<()>;
    fn stop(&self) -> common::error::Result<()>;
    fn pause(&self) -> common::error::Result<()>;
    fn resume(&self) -> common::error::Result<()>;

    // Reactivity
    fn on_excitement_source_changed(&self, source: ExcitementSource) -> common::error::Result<()>;
    fn on_layout_changed(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        tuner_config: &TunerConfig
    ) -> common::error::Result<()>;

    // Data taps
    fn snapshot_output_snoop(&self, key: NodeKey) -> Vec<f32>;
    fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)>;
    fn snapshot_excitement_snoop(&self, key: NodeKey) -> Vec<(f32, f32)>;
    fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)>;
    fn snapshot_processed_output_spectrum(&self) -> common::error::Result<Option<ProcessedOutputSpectrumSnapshot>>;

    // Band control
    fn set_band_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()>;

    fn get_band_control(&self, key: common::NodeKey) -> common::error::Result<f32>;

    // Key control
    fn set_key_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()>;

    fn get_key_control(&self, key: common::NodeKey) -> common::error::Result<f32>;

    // Fine-tuned values (editor feature)
    #[cfg(feature = "editor")]
    fn get_finetuned_values(&self) -> common::error::Result<common::commands::edit::FineTunedValuesPayload>;

    #[cfg(feature = "editor")]
    fn set_finetuned_values(
        &self,
        payload: common::commands::edit::FineTunedValuesPayload
    ) -> common::error::Result<()>;

    // Tuner integration
    fn start_tuner_only(&self, tuner_config: &TunerConfig) -> common::error::Result<()>;
    fn poll_tuner_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot>;
    fn start_tap_tuner_audio(&self) -> common::error::Result<()>;
    fn stop_tap_tuner_audio(&self) -> common::error::Result<()>;
    fn update_tuner_config(&self, tuner_config: &TunerConfig) -> common::error::Result<()>;
    fn poll_tuner_excitements(&self) -> Vec<(NodeKey, f32)>;
    fn get_sample_rate(&self) -> f64;

    fn is_batch_processing(&self) -> bool;
}

/// Null / no-op runtime used when no concrete backend feature is enabled.
///
/// Provides graceful degradation:
/// - Calls succeed so UI logic stays consistent.
/// - Data taps return empty vectors.
/// - No threads or devices are opened.
#[derive(Debug, Default)]
pub struct NullController;

impl AudioRuntime for NullController {
    #[cfg(feature = "editor")]
    fn get_finetuned_values(&self) -> common::error::Result<common::commands::edit::FineTunedValuesPayload> {
        Err(common::error::InstrumentError::NotInitialized.into())
    }

    #[cfg(feature = "editor")]
    #[allow(clippy::too_many_arguments)]
    fn set_finetuned_values(
        &self,
        _payload: common::commands::edit::FineTunedValuesPayload
    ) -> common::error::Result<()> {
        Ok(())
    }

    fn start(&self, _layout: &InstrumentLayout,
    _config: &InstrumentConfig,
    _source: ExcitementSource,
    _tuner_config: &TunerConfig,) -> common::error::Result<()> {
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

    fn on_excitement_source_changed(&self, _source: ExcitementSource) -> common::error::Result<()> {
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

    fn snapshot_output_snoop(&self, _key: NodeKey) -> Vec<f32> {
        Vec::new()
    }

    fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
        Vec::new()
    }

    fn snapshot_excitement_snoop(&self, _key: NodeKey) -> Vec<(f32, f32)> {
        Vec::new()
    }

    fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)> {
        Vec::new()
    }

    fn snapshot_processed_output_spectrum(&self) -> common::error::Result<Option<(BTreeMap<u32, f32>, BTreeMap<u32, f32>)>> {
        Ok(None)
    }

    fn set_band_control(&self, _key: common::NodeKey, _value: f32) -> common::error::Result<()> {
        Ok(())
    }

    fn get_band_control(&self, _key: common::NodeKey) -> common::error::Result<f32> {
        Ok(0.0)
    }

    fn set_key_control(&self, _key: common::NodeKey, _value: f32) -> common::error::Result<()> {
        Ok(())
    }

    fn get_key_control(&self, _key: common::NodeKey) -> common::error::Result<f32> {
        Ok(0.0)
    }

    fn start_tuner_only(&self, _tuner_config: &TunerConfig) -> common::error::Result<()> {
        Ok(())
    }

    fn update_tuner_config(&self, _tuner_config: &TunerConfig) -> common::error::Result<()> {
        Ok(())
    }

    fn start_tap_tuner_audio(&self) -> common::error::Result<()> {
        Ok(())
    }

    fn stop_tap_tuner_audio(&self) -> common::error::Result<()> {
        Ok(())
    }

    fn poll_tuner_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot> {
        None
    }

    fn poll_tuner_excitements(&self) -> Vec<(NodeKey, f32)> {
        vec![]
    }

    fn get_sample_rate(&self) -> f64 {
        44100.0
    }

    fn is_batch_processing(&self) -> bool {
        false
    }
}

/// Factory returning the highest-precedence available runtime.
///
/// Precedence (current):
/// 1. rt_cpal
/// 2. rt_web
/// 3. NullController (fallback)
#[allow(unreachable_code)]
pub fn make_stream_controller() -> common::error::Result<Box<dyn AudioRuntime + Send + Sync>> {
    #[cfg(feature = "rt_cpal")]
    {
        return cpal::make_stream_controller();
    }
    #[cfg(all(not(feature = "rt_cpal"), feature = "rt_web"))]
    {
        return web::make_stream_controller();
    }
    Ok(Box::new(NullController))
}
/// Indicates whether the active build includes a microphone-capable backend.
#[allow(unreachable_code)]
#[allow(clippy::needless_return)]
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
#[allow(clippy::needless_return)]
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
