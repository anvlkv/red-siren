#[cfg(feature = "rt_cpal")]
pub mod cpal;
pub mod gate_manager;
#[cfg(feature = "rt_web")]
pub mod web;

mod rt_subsystem;
pub mod telemetry;

use std::collections::BTreeMap;

use common::NodeKey;
use common::device::DeviceData;
use common::instrument::{
    Config as InstrumentConfig, Layout as InstrumentLayout, PlaybackQuality, Preset,
};
use common::tuner::Config as TunerConfig;

use crate::quality::{PlaybackQualityGate, SampleType};
use crate::rt::telemetry::TelemetrySender;

pub type ProcessedOutputSpectrumSnapshot = (BTreeMap<u32, f32>, BTreeMap<u32, f32>);

/// Source of excitement energy driving instrument strings / nodes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ExcitementSource {
    /// Pseudo-random / noise entropy (internal generator).
    #[default]
    Entropy,
    /// Live microphone input (if permission & capture available).
    Mic,
    /// Manually excite any specific node
    Manual
}

/// Buffer processing mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessingMode {
    None,
    #[default]
    Tick,
    Process,
    ProcessBig,
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
            ExcitementSource::Manual => 2,
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
        preset: Preset,
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
        tuner_config: &TunerConfig,
    ) -> common::error::Result<()>;

    // Data taps
    fn snapshot_output_snoop(&self, key: NodeKey) -> Vec<f32>;
    fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)>;
    fn snapshot_excitement_snoop(&self, key: NodeKey) -> Vec<(f32, f32)>;
    fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)>;
    fn snapshot_processed_output_spectrum(
        &self,
    ) -> common::error::Result<Option<ProcessedOutputSpectrumSnapshot>>;
    fn snapshot_input_snoop(&self) -> Vec<f32>;

    // Band control
    fn set_band_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()>;

    fn get_band_control(&self, key: common::NodeKey) -> common::error::Result<f32>;

    // Key control
    fn set_key_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()>;

    fn get_key_control(&self, key: common::NodeKey) -> common::error::Result<f32>;

    /// Set excite values + frequency + key for a test hit (devtools feature).
    fn hit_test_node(&self, key: NodeKey, frequency: f32, excite_real: f32, excite_imag: f32) -> common::error::Result<()>;

    /// Release a test node (sets key control to 0.0, resets excite to 0).
    fn release_test_node(&self, key: NodeKey) -> common::error::Result<()>;

    // Fine-tuned values (editor feature)
    #[cfg(feature = "editor")]
    fn get_finetuned_values(
        &self,
    ) -> common::error::Result<common::commands::edit::FineTunedValuesPayload>;

    #[cfg(feature = "editor")]
    fn set_finetuned_values(
        &self,
        payload: common::commands::edit::FineTunedValuesPayload,
    ) -> common::error::Result<()>;

    // Tuner integration
    fn start_tuner_only(&self, tuner_config: &TunerConfig) -> common::error::Result<()>;
    fn poll_tuner_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot>;
    fn start_tap_tuner_audio(&self) -> common::error::Result<()>;
    fn stop_tap_tuner_audio(&self) -> common::error::Result<()>;
    fn update_tuner_config(&self, tuner_config: &TunerConfig) -> common::error::Result<()>;
    fn poll_tuner_excitements(&self) -> Vec<(NodeKey, f32)>;
    fn get_sample_rate(&self) -> f64;

    fn quality_indicator(&self) -> PlaybackQuality;

    fn update_quality_setting(&self, qg: PlaybackQualityGate);

    fn set_preset(&self, preset: Preset) -> common::error::Result<()>;

    fn get_preset(&self) -> Preset;

    /// Whether the output stream is currently active (CPAL stream thread running).
    /// Default implementation returns `false` (suitable for NullController).
    fn is_running(&self) -> bool {
        false
    }

    /// The [`SampleType`] (F32 or F64) currently used by the running DSP network.
    /// Default implementation returns `SampleType::F32`.
    fn current_sample_type(&self) -> SampleType {
        SampleType::default()
    }

    /// Restart the output stream to apply a pending quality-gate change that
    /// requires a DSP rebuild (e.g. F32 ↔ F64 sample-type transition).
    ///
    /// Implementations should be a no-op when the stream is not running.
    /// Default returns `Ok(())` (suitable for NullController).
    fn restart_for_quality(&self) -> common::error::Result<()> {
        Ok(())
    }
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
    fn get_finetuned_values(
        &self,
    ) -> common::error::Result<common::commands::edit::FineTunedValuesPayload> {
        Err(common::error::InstrumentError::NotInitialized.into())
    }

    #[cfg(feature = "editor")]
    fn set_finetuned_values(
        &self,
        _payload: common::commands::edit::FineTunedValuesPayload,
    ) -> common::error::Result<()> {
        Ok(())
    }

    fn start(
        &self,
        _layout: &InstrumentLayout,
        _config: &InstrumentConfig,
        _source: ExcitementSource,
        _tuner_config: &TunerConfig,
        _preset: Preset,
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

    fn on_excitement_source_changed(&self, _source: ExcitementSource) -> common::error::Result<()> {
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

    fn snapshot_input_snoop(&self) -> Vec<f32> {
        Vec::new()
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

    fn snapshot_processed_output_spectrum(
        &self,
    ) -> common::error::Result<Option<(BTreeMap<u32, f32>, BTreeMap<u32, f32>)>> {
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

    fn hit_test_node(&self, _key: NodeKey, _frequency: f32, _excite_real: f32, _excite_imag: f32) -> common::error::Result<()> {
        Ok(())
    }

    fn release_test_node(&self, _key: NodeKey) -> common::error::Result<()> {
        Ok(())
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

    fn quality_indicator(&self) -> PlaybackQuality {
        PlaybackQuality::default()
    }

    fn update_quality_setting(&self, _qg: PlaybackQualityGate) {}

    fn set_preset(&self, _preset: Preset) -> common::error::Result<()> {
        Ok(())
    }

    fn get_preset(&self) -> Preset {
        Preset::default()
    }
}

/// Factory returning the highest-precedence available runtime.
///
/// Precedence (current):
/// 1. rt_cpal
/// 2. rt_web
/// 3. NullController (fallback)
#[allow(unreachable_code)]
pub fn make_stream_controller(
    telemetry: TelemetrySender,
    preset: Option<Preset>,
    output_device: Option<DeviceData>,
    input_device: Option<DeviceData>,
) -> common::error::Result<Box<dyn AudioRuntime + Send + Sync>> {
    #[cfg(feature = "rt_cpal")]
    {
        return cpal::make_stream_controller(telemetry, preset, output_device, input_device);
    }
    #[cfg(all(not(feature = "rt_cpal"), feature = "rt_web"))]
    {
        return web::make_stream_controller(telemetry, preset, output_device, input_device);
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
