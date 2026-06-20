use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use thingbuf::mpsc::{channel, Receiver, Sender};

use super::gate::{PlaybackQualityGate, SampleType};

pub type TelemetrySender = Arc<Sender<Message>>;
pub type TelemetryReceiver = Arc<Receiver<Message>>;

pub fn create_telemetry_channel() -> (TelemetrySender, TelemetryReceiver) {
    let (sx, rx) = channel(128);
    (Arc::new(sx), Arc::new(rx))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Message {
    pub buffer_size: usize,
    pub estimated_latency: Option<Duration>,
    pub sample_rate: u32,
    pub input_latency: Option<Duration>,
}

impl Message {
    fn buffer_duration(&self) -> Duration {
        Duration::from_secs_f64(self.buffer_size as f64 / self.sample_rate as f64)
    }
}

#[derive(Clone)]
pub struct PlaybackTelemetry {
    history: VecDeque<Message>,
    running_duration: Duration,
    history_duration: Duration,
}

impl PlaybackTelemetry {
    pub fn new(history_duration: Duration) -> Self {
        Self {
            history: VecDeque::new(),
            running_duration: Duration::ZERO,
            history_duration,
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.running_duration = Duration::ZERO;
    }

    pub fn add_message(&mut self, msg: Message) {
        self.running_duration += msg.buffer_duration();
        self.history.push_back(msg);
        self.trim_history();
    }

    pub fn avg_latency(&self) -> Option<Duration> {
        if self.history.is_empty() {
            None
        } else {
            Some(
                self.history.iter().fold(Duration::ZERO, |acc, msg| {
                    acc + msg.estimated_latency.unwrap_or_default()
                }) / self.history.len() as u32,
            )
        }
    }

    pub fn avg_input_latency(&self) -> Option<Duration> {
        if self.history.is_empty() {
            None
        } else {
            Some(
                self.history.iter().fold(Duration::ZERO, |acc, msg| {
                    acc + msg.input_latency.unwrap_or_default()
                }) / self.history.len() as u32,
            )
        }
    }

    pub fn running_duration(&self) -> Duration {
        self.running_duration
    }

    fn trim_history(&mut self) {
        let mut total_duration = self.running_duration;

        while total_duration > self.history_duration {
            if let Some(front) = self.history.pop_front() {
                total_duration -= front.buffer_duration();
            } else {
                break;
            }
        }
        self.running_duration = total_duration;
    }
}
