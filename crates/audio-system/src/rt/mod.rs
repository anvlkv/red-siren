pub mod gate_manager;
pub mod telemetry;
pub mod stream;

use crate::quality::{PlaybackQualityGate, SampleType};

use telemetry::TelemetrySender;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ExcitementSource {
    #[default]
    Entropy,
    Mic,
}

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
    fn from(value: ExcitementSource) -> Self {
        match value {
            ExcitementSource::Entropy => 0,
            ExcitementSource::Mic => 1,
        }
    }
}

pub trait AudioRuntime {
    fn start(&self) -> Result<(), String> {
        Ok(())
    }
    fn stop(&self) -> Result<(), String> {
        Ok(())
    }
    fn pause(&self) -> Result<(), String> {
        Ok(())
    }
    fn resume(&self) -> Result<(), String> {
        Ok(())
    }
    fn quality_indicator(&self) -> PlaybackQualityGate {
        PlaybackQualityGate::default()
    }
    fn update_quality_setting(&self, _gate: PlaybackQualityGate) {}
    fn is_running(&self) -> bool {
        false
    }
    fn current_sample_type(&self) -> SampleType {
        SampleType::default()
    }
    fn restart_for_quality(&self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NullController;

impl AudioRuntime for NullController {}

pub fn make_stream_controller(_telemetry: TelemetrySender) -> Box<dyn AudioRuntime + Send + Sync> {
    Box::new(NullController)
}

pub fn supports_mic() -> bool {
    false
}

pub async fn check_mic_permission() -> Result<(), String> {
    Err("microphone runtime is unavailable in reset scaffold".to_string())
}
