use std::sync::Arc;

use au_core::UnitResolve;
use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use futures::{channel::mpsc::UnboundedReceiver, StreamExt};
use serde::{Deserialize, Serialize};
use parking_lot::Mutex;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PlayOperation {
    RunUnit,
    Permissions,
}

impl Operation for PlayOperation {
    type Output = UnitResolve;
}

#[derive(Capability)]
pub struct Play<Ev> {
    context: CapabilityContext<PlayOperation, Ev>,
    receiver: Arc<Mutex<Option<UnboundedReceiver<UnitResolve>>>>,
}

impl<Ev> Play<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<PlayOperation, Ev>) -> Self {
        Self {
            context,
            receiver: Default::default(),
        }
    }

    pub fn with_receiver(&self, receiver: UnboundedReceiver<UnitResolve>) {
        let mut recv_option = self.receiver.lock();
        _ = recv_option.insert(receiver);
    }

    pub fn run_unit<F>(&self, notify: F)
    where
        F: Fn(UnitResolve) -> Ev + Send + 'static,
    {
        let context = self.context.clone();
        let mut receiver = self.receiver.lock();
        let mut receiver = receiver.take().unwrap();
        self.context.spawn({
            async move {
                context.notify_shell(PlayOperation::RunUnit).await;
                while let Some(resolve) = receiver.next().await {
                    context.update_app(notify(resolve))
                }
            }
        });
    }
}
