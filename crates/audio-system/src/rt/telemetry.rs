use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use crate::quality::{PlaybackQualityGate, SampleType};

use super::ProcessingMode;

pub type TelemetrySender = Sender<Message>;
pub type TelemetryReceiver = Receiver<Message>;

pub fn create_telemetry_channel() -> (TelemetrySender, TelemetryReceiver) {
    channel()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Message {
    pub mode: ProcessingMode,
    pub filled_size: usize,
    pub buffer_size: usize,
    pub processing_time: Duration,
    pub estimated_latency: Option<Duration>,
    pub quality: PlaybackQualityGate,
    pub sample_type: SampleType,
}
