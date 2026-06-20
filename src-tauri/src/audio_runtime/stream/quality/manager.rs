use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;

use super::gate::{PlaybackQuality, PlaybackQualityGate};
use super::telemetry::{PlaybackTelemetry, TelemetryReceiver};

#[derive(Clone)]
pub struct QualityGateManger {
    current_quality: Arc<RwLock<PlaybackQuality>>,
    current_gate: PlaybackQualityGate,
    telemetry_receiver: TelemetryReceiver,
    telemetry: PlaybackTelemetry,
}

impl QualityGateManger {
    const HISTORY_DURATION_S: f64 = 0.75;
    const INPUT_LATENCY_RATIO_THRESHOLD: f64 = 0.75;
    const UPGRADE_LATENCY_RATIO_THRESHOLD: f64 = 0.1;
    const DOWNGRADE_LATENCY_RATIO_THRESHOLD: f64 = 0.5;

    pub fn new(
        telemetry_receiver: TelemetryReceiver,
        current_quality: Arc<RwLock<PlaybackQuality>>,
    ) -> Self {
        Self {
            current_quality,
            telemetry_receiver,
            current_gate: PlaybackQualityGate::default(),
            telemetry: PlaybackTelemetry::new(Duration::from_secs_f64(Self::HISTORY_DURATION_S)),
        }
    }

    pub async fn run(&mut self, update: impl Fn(PlaybackQuality) -> bool + Send + Sync + 'static) {
        while let Some(msg) = self.telemetry_receiver.recv().await {
            self.telemetry.add_message(msg);
            let proposed_quality = self.evaluate_quality();
            if let Some(new_quality) = proposed_quality {
                self.current_quality.write().clone_from(&new_quality);
                self.current_gate = PlaybackQualityGate::from(new_quality);
                if update(new_quality) {
                    self.telemetry.clear();
                }
            }
        }
    }

    fn evaluate_quality(&self) -> Option<PlaybackQuality> {
        let current_quality = *self.current_quality.read();
        let current_gate = self.current_gate;
        let derived_gate = PlaybackQualityGate::from(current_quality);
        if derived_gate != current_gate {
            let new_quality = PlaybackQuality::from(derived_gate);
            Some(new_quality)
        } else if matches!(current_quality, PlaybackQuality::Auto(_))
            && self.telemetry.running_duration().as_secs_f64() >= Self::HISTORY_DURATION_S
        {
            let input_latency = self.telemetry.avg_input_latency();
            self.telemetry.avg_latency().and_then(|lat| {
                let latency_ratio = lat.as_secs_f64() / Self::HISTORY_DURATION_S;
                let input_latency_ratio = input_latency
                    .map(|input_lat| input_lat.as_secs_f64() / Self::HISTORY_DURATION_S)
                    .unwrap_or(0.0);
                match (latency_ratio, input_latency_ratio) {
                    (r, i)
                        if r < Self::UPGRADE_LATENCY_RATIO_THRESHOLD
                            && i < Self::INPUT_LATENCY_RATIO_THRESHOLD =>
                    {
                        let upgraded_gate = current_gate.upgrade();
                        Some(PlaybackQuality::Auto(upgraded_gate))
                    }
                    (r, i)
                        if r > Self::DOWNGRADE_LATENCY_RATIO_THRESHOLD
                            || i > Self::INPUT_LATENCY_RATIO_THRESHOLD =>
                    {
                        let downgraded_gate = current_gate.downgrade();
                        Some(PlaybackQuality::Auto(downgraded_gate))
                    }
                    _ => None,
                }
            })
        } else {
            None
        }
        .filter(|q| *q != current_quality)
    }
}
