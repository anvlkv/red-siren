use std::{
    ops::DerefMut,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use common::instrument::PlaybackQuality;
use fundsp::DEFAULT_SR;
use parking_lot::{Mutex, RwLock};
use tokio::{
    runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime},
    task::{self, JoinHandle as TokioJoinHandle},
};

use crate::quality::{PlaybackQualityGate, SampleType};

use super::telemetry::{PlaybackTelemetry, TelemetryReceiver};

/// Drives playback-quality adjustments on a dedicated Tokio runtime so we never
/// rely on a globally installed executor during Tauri setup.
pub struct QualityGateManager {
    telemetry: Arc<Mutex<PlaybackTelemetry>>,
    quality_setting: Arc<RwLock<PlaybackQuality>>,
    proposed_quality_gate: Arc<RwLock<PlaybackQualityGate>>,
    shutdown: Arc<AtomicBool>,
    worker: Option<GateWorker>,
}

impl QualityGateManager {
    pub fn new(
        telemetry_rx: TelemetryReceiver,
        managed_change: Box<dyn FnMut(PlaybackQualityGate) + Send + Sync>,
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
        let shutdown = Arc::new(AtomicBool::new(false));

        let worker = GateWorker::spawn(
            telemetry_rx,
            telemetry.clone(),
            quality_setting.clone(),
            proposed_quality_gate.clone(),
            shutdown.clone(),
            Arc::new(Mutex::new(managed_change)),
        );

        Self {
            telemetry,
            quality_setting,
            proposed_quality_gate,
            shutdown,
            worker: Some(worker),
        }
    }

    pub fn proposed_quality(&self) -> PlaybackQuality {
        (*self.proposed_quality_gate.read()).into()
    }

    pub fn current_quality(&self) -> PlaybackQuality {
        *self.quality_setting.read()
    }

    /// Override the quality setting. When set to [`PlaybackQuality::Auto`], the
    /// gate worker will resume managing quality automatically via telemetry.
    /// When set to a specific quality, the gate worker will stop updating
    /// (the `Auto` branch won't match) until switched back to Auto.
    pub fn set_quality(&self, quality: PlaybackQuality) {
        let prev = *self.quality_setting.read();
        log::info!(
            "QualityGateManager::set_quality: {:?} → {:?}",
            prev,
            quality
        );
        *self.quality_setting.write() = quality;
    }

    pub fn update_settings(
        &self,
        sample_rate: u32,
        sample_type: SampleType,
        quality: PlaybackQuality,
        buffer_size: Option<usize>,
    ) {
        let gate = PlaybackQualityGate::from(quality);
        let mut telemetry = self.telemetry.lock();
        telemetry.update(sample_rate, sample_type, gate, buffer_size);
        *self.quality_setting.write() = quality;
    }
}

impl Drop for QualityGateManager {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.stop();
        }
    }
}

struct GateWorker {
    runtime: TokioRuntime,
    handle: TokioJoinHandle<()>,
}

impl GateWorker {
    #[allow(clippy::too_many_arguments)]
    fn spawn(
        telemetry_rx: TelemetryReceiver,
        telemetry: Arc<Mutex<PlaybackTelemetry>>,
        quality_setting: Arc<RwLock<PlaybackQuality>>,
        proposed_quality_gate: Arc<RwLock<PlaybackQualityGate>>,
        shutdown: Arc<AtomicBool>,
        managed_change: Arc<Mutex<Box<dyn FnMut(PlaybackQualityGate) + Send + Sync>>>,
    ) -> Self {
        let runtime = TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("failed to build quality gate runtime");

        let handle = runtime.spawn(async move {
            loop {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }

                let Some(msg) = telemetry_rx.recv().await else {
                    break;
                };

                if let Some(change) = {
                    let mut tel = telemetry.lock();
                    tel.accept_message(msg)
                } {
                    let prev_proposed = *proposed_quality_gate.read();
                    if prev_proposed != change {
                        log::debug!(
                            "GateWorker: proposed quality gate changed {:?} → {:?}",
                            prev_proposed,
                            change
                        );
                    }
                    *proposed_quality_gate.write() = change;

                    let mut qs = quality_setting.write();
                    if let PlaybackQuality::Auto(value) = qs.deref_mut() {
                        let prev_gate = PlaybackQualityGate::from(*value);
                        *value = change as i8;
                        if prev_gate != change {
                            log::info!(
                                "GateWorker: auto quality adjusted {:?} → {:?} (gate i8={})",
                                prev_gate,
                                change,
                                *value
                            );
                        }
                        (managed_change.lock())(PlaybackQualityGate::from(*value));
                    } else {
                        log::debug!(
                            "GateWorker: telemetry suggests {:?} but quality is manually pinned to {:?}; skipping",
                            change,
                            *qs
                        );
                    }
                } else {
                    task::yield_now().await;
                }
            }
        });

        Self { runtime, handle }
    }

    fn stop(self) {
        self.handle.abort();
        self.runtime.shutdown_background();
    }
}
