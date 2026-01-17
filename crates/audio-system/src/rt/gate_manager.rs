use std::{
    ops::DerefMut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
};

use common::instrument::PlaybackQuality;
use fundsp::{thingbuf::ThingBuf, DEFAULT_SR};
use parking_lot::{Mutex, RwLock};

use crate::quality::{PlaybackQualityGate, SampleType};

use super::telemetry::*;

pub struct QualityGateManager {
    channel: Arc<ThingBuf<Message>>,
    verdict_channel: Arc<ThingBuf<PlaybackQualityGate>>,
    telemetry: Arc<Mutex<PlaybackTelemetry>>,
    quality_setting: Arc<RwLock<PlaybackQuality>>,
    proposed_quality_gate: Arc<RwLock<PlaybackQualityGate>>,
    job_running: Arc<AtomicBool>,
    job: Option<JoinHandle<()>>,
}

impl QualityGateManager {
    pub fn new(
        channel: Arc<ThingBuf<Message>>,
        verdict_channel: Arc<ThingBuf<PlaybackQualityGate>>,
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
        let job = thread::spawn({
            let telemetry = telemetry.clone();
            let quality_setting = quality_setting.clone();
            let proposed_quality_gate = proposed_quality_gate.clone();
            let job_running = job_running.clone();
            let channel = channel.clone();
            let verdict_channel = verdict_channel.clone();
            move || loop {
                if !job_running.load(Ordering::SeqCst) {
                    break;
                }

                if let Some(msg) = channel.pop() {
                    let mut tel = telemetry.lock();
                    if let Some(change) = tel.accept_message(msg) {
                        *proposed_quality_gate.write() = change;
                        let mut qs = quality_setting.write();
                        match qs.deref_mut() {
                            PlaybackQuality::Auto(val) => {
                                *val = change as i8;
                                if let Err(e) =
                                    verdict_channel.push(PlaybackQualityGate::from(*val))
                                {
                                    _ = verdict_channel.pop().unwrap();
                                    verdict_channel.push(e.into_inner()).unwrap();
                                }
                            }
                            _ => {}
                        }
                    } else {
                        thread::yield_now();
                    }
                }
            }
        });

        Self {
            channel,
            verdict_channel,
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
        if let Err(e) = self.verdict_channel.push(q_gate) {
            _ = self.verdict_channel.pop().unwrap();
            self.verdict_channel.push(e.into_inner()).unwrap();
        }
    }
}

impl Drop for QualityGateManager {
    fn drop(&mut self) {
        self.job_running.store(false, Ordering::SeqCst);
        self.job.take().and_then(|job| job.join().ok()).unwrap();
    }
}
