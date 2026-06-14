use std::sync::Arc;

use common::error::{InstrumentError, Result};
use fundsp::{
    prelude::*,
    thingbuf::mpsc::{channel, Receiver, Sender},
};
use num_rational::Rational32;

use crate::system::grid::FrameEncodedSignal;

use super::scheduler::SchedulingRequest;

#[derive(Clone)]
pub struct SchedulingRequestEncoder {
    receiver: Arc<Receiver<SchedulingRequest>>,
}

impl AudioNode for SchedulingRequestEncoder {
    const ID: u64 = crate::util::hash_str(concat!(module_path!(), "::SchedulingRequestEncoder"));

    type Inputs = U0;

    type Outputs = <SchedulingRequest as FrameEncodedSignal>::Size;

    fn tick(&mut self, _input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let scheduling_request = self.receiver.try_recv().unwrap_or(SchedulingRequest::None);
        scheduling_request.encode()
    }
}

#[derive(Clone)]
pub struct SchedulingRequestEncoderHandle {
    sender: Arc<Sender<SchedulingRequest>>,
}

impl SchedulingRequestEncoderHandle {
    pub fn send(
        &self,
        time: Rational32,
        duration: Rational32,
        repeat: Option<u32>,
        event: f64,
    ) -> Result<()> {
        let request = SchedulingRequest::Request {
            time,
            duration,
            repeat,
            event,
        };
        self.sender.try_send(request).map_err(|e| match e {
            TrySendError::Full(_) => InstrumentError::ExciteChannelFull.into(),
            TrySendError::Closed(_) => InstrumentError::ExciteChannelClosed.into(),
            _ => InstrumentError::Other(format!(
                "unexpected error sending scheduling request: {}",
                e
            ))
            .into(),
        })
    }
}

pub fn create_scheduling_request_encoder(
) -> (SchedulingRequestEncoderHandle, An<SchedulingRequestEncoder>) {
    let (sender, receiver) = channel(16);
    let encoder = SchedulingRequestEncoder {
        receiver: Arc::new(receiver),
    };
    let handle = SchedulingRequestEncoderHandle {
        sender: Arc::new(sender),
    };
    (handle, An(encoder))
}
