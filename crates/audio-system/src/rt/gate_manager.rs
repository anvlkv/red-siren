use std::sync::{Arc, Mutex, RwLock};

use crate::quality::{PlaybackQualityGate, SampleType};

use super::telemetry::TelemetryReceiver;

pub struct QualityGateManager {
    quality_setting: Arc<RwLock<PlaybackQualityGate>>,
    proposed_quality_gate: Arc<RwLock<PlaybackQualityGate>>,
    managed_change: Arc<Mutex<Box<dyn FnMut(PlaybackQualityGate) + Send + Sync>>>,
}

impl QualityGateManager {
    pub fn new(
        _telemetry_rx: TelemetryReceiver,
        managed_change: Box<dyn FnMut(PlaybackQualityGate) + Send + Sync>,
    ) -> Self {
        Self {
            quality_setting: Arc::new(RwLock::new(PlaybackQualityGate::default())),
            proposed_quality_gate: Arc::new(RwLock::new(PlaybackQualityGate::default())),
            managed_change: Arc::new(Mutex::new(managed_change)),
        }
    }

    pub fn proposed_quality(&self) -> PlaybackQualityGate {
        *self
            .proposed_quality_gate
            .read()
            .expect("proposed quality lock poisoned")
    }

    pub fn current_quality(&self) -> PlaybackQualityGate {
        *self
            .quality_setting
            .read()
            .expect("quality setting lock poisoned")
    }

    pub fn set_quality(&self, quality: PlaybackQualityGate) {
        {
            let mut setting = self
                .quality_setting
                .write()
                .expect("quality setting lock poisoned");
            *setting = quality;
        }
        {
            let mut proposed = self
                .proposed_quality_gate
                .write()
                .expect("proposed quality lock poisoned");
            *proposed = quality;
        }

        let mut callback = self
            .managed_change
            .lock()
            .expect("managed change lock poisoned");
        (callback)(quality);
    }

    pub fn update_settings(
        &self,
        _sample_rate: u32,
        _sample_type: SampleType,
        quality: PlaybackQualityGate,
        _buffer_size: Option<usize>,
    ) {
        self.set_quality(quality);
    }
}
