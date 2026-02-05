use std::{
    ops::DerefMut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use common::instrument::PlaybackQuality;
use fundsp::{
    thingbuf::mpsc::{Receiver, Sender, channel},
    DEFAULT_SR,
};
use tokio::{task::{JoinHandle, self}};
use parking_lot::{Mutex, RwLock};

use crate::quality::{PlaybackQualityGate, SampleType};

use super::telemetry::*;

pub struct QualityGateManager {
    telemetry: Arc<Mutex<PlaybackTelemetry>>,
    quality_setting: Arc<RwLock<PlaybackQuality>>,
    proposed_quality_gate: Arc<RwLock<PlaybackQualityGate>>,
    job_running: Arc<AtomicBool>,
    job: Option<JoinHandle<()>>,
}

impl QualityGateManager {
    pub fn new(
        telemetry_rx: TelemetryReceiver,
        mut managed_change: Box<dyn FnMut(PlaybackQualityGate) + Send + Sync>
    ) -> Self {
        let proposed_quality_gate = PlaybackQualityGate::default();
        let telemetry = Arc::new(Mutex::new(PlaybackTelemetry::new(
            DEFAULT_SR as u32,
            proposed_quality_gate.sample_type(),
            proposed_quality_gate,
            None,
        )));
        let quality_setting = Arc::new(RwLock::new(PlaybackQuality::default()));
        let proposed_quality_gate = Arc::new(RwLock::new(proposed_quality_gate));
        let job_running = Arc::new(AtomicBool::new(true));
        let job = task::spawn({
            let telemetry = telemetry.clone();
            let quality_setting = quality_setting.clone();
            let proposed_quality_gate = proposed_quality_gate.clone();
            let job_running = job_running.clone();
            async move {
                loop {
                    if !job_running.load(Ordering::SeqCst) {
                        break;
                    }

                    if let Some(msg) = telemetry_rx.recv().await {

                        if let Some(change) = {
                            let mut tel = telemetry.lock();
                            tel.accept_message(msg)
                        } {
                            *proposed_quality_gate.write() = change;
                            let mut qs = quality_setting.write();
                            if let PlaybackQuality::Auto(val) = qs.deref_mut() {
                                *val = change as i8;
                                managed_change(PlaybackQualityGate::from(*val));
                            }
                        } else {
                            task::yield_now().await;
                        }
                    }
                }
            }
        });

        Self {
            telemetry,
            quality_setting,
            proposed_quality_gate,
            job_running,
            job: Some(job),
        }
    }

    pub fn proposed_quality(&self) -> PlaybackQuality {
        (*self.proposed_quality_gate.read()).into()
    }

    pub fn current_quality(&self) -> PlaybackQuality {
        *self.quality_setting.read()
    }

    pub fn update_settings(
        &self,
        sample_rate: u32,
        sample_type: SampleType,
        quality: PlaybackQuality,
        buffer_size: Option<usize>,
    ) {
        let q_gate = PlaybackQualityGate::from(quality);
        let mut telemetry = self.telemetry.lock();
        telemetry.update(sample_rate, sample_type, q_gate, buffer_size);
        *self.quality_setting.write() = quality;
    }
}

impl Drop for QualityGateManager {
    fn drop(&mut self) {
        self.job_running.store(false, Ordering::SeqCst);
        if let Some(j) = self.job.take() {
            j.abort();
        }
    }
}
