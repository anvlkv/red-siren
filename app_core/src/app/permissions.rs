use std::sync::Arc;

use anyhow::Result;
use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver},
    StreamExt,
};
use parking_lot::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PermissionsOperation {
    AudioRecording,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PermissionsOperationOutput {
    Granted,
    Rejected,
    PermanentlyRejected,
}

impl Operation for PermissionsOperation {
    type Output = PermissionsOperationOutput;
}

#[derive(Capability)]
pub struct Permissions<Ev> {
    context: CapabilityContext<PermissionsOperation, Ev>,
}

impl<Ev> Permissions<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<PermissionsOperation, Ev>) -> Self {
        Self { context }
    }

    pub fn recording_permission<F>(&self, notify: F)
    where
        F: Fn(Option<bool>) -> Ev + Send + 'static,
    {
        let context = self.context.clone();
        self.context.spawn({
            async move {
                let resolve = match context
                    .request_from_shell(PermissionsOperation::AudioRecording)
                    .await
                {
                    PermissionsOperationOutput::Granted => Some(true),
                    PermissionsOperationOutput::Rejected => None,
                    PermissionsOperationOutput::PermanentlyRejected => Some(false),
                };

                context.update_app(notify(resolve));
            }
        });
    }
}
