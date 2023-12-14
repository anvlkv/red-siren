use futures::channel::mpsc::UnboundedSender;

use shared::play::{PlayOperation, PlayOperationOutput};

pub struct CoreStreamer;

impl Default for CoreStreamer {
    fn default() -> Self {
        Self
    }
}

impl super::StreamerUnit for CoreStreamer {
    fn update(&self, event: PlayOperation, rx: UnboundedSender<PlayOperationOutput>) {}
}
