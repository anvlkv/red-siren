//! Shared audio types and traits
//!
//! PURPOSE
//! -------
//! Core audio types that need to be shared between src-tauri and audio-system.
//! This enables the Web Audio migration where src-tauri implements StreamController
//! without depending on audio-system as a runtime dependency.
//!
//! MAYA DRY KISS
//! -------------
//! - Simple, focused types without implementation details
//! - Clear separation between interface and implementation
//! - No dependencies on specific audio backends

use serde::{Deserialize, Serialize};

use crate::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use crate::tuner::Config as TunerConfig;

/// Source of activation energy driving instrument strings / nodes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Unified interface for audio stream control.
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
    ) -> crate::error::Result<()>;
    fn stop(&self) -> crate::error::Result<()>;
    fn pause(&self) -> crate::error::Result<()>;
    fn resume(&self) -> crate::error::Result<()>;

    // Reactivity
    fn on_activation_source_changed(&self, source: ActivationSource) -> crate::error::Result<()>;
    fn on_layout_changed(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        tuner_config: &TunerConfig,
    ) -> crate::error::Result<()>;

    // Data snoops (per-string sample snapshots)
    fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32>;
    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)>;
    fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32>;
    fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)>;

    // Band control (per-string parameter adjustment)
    fn set_band_control(&self, group: u8, key: u8, value: f32) -> crate::error::Result<()>;
}

/// Interface for tuner runtime implementations.
pub trait TunerRuntime: Send + Sync {
    fn start(&self, config: &crate::tuner::Config) -> crate::error::Result<()>;
    fn stop(&self);
    fn update_config(&self, config: &crate::tuner::Config);
    /// Poll for latest spectrum data (non-blocking). Returns None if not ready.
    fn poll_spectrum(&self) -> Option<crate::tuner::SpectrumData>;
}
