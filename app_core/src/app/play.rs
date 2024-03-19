use std::sync::Arc;

pub use au_core::UnitResolve;
use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use futures::{channel::mpsc::UnboundedReceiver, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

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

    pub fn recording_permission<F>(&self, notify: F)
    where
        F: Fn(UnitResolve) -> Ev + Send + 'static,
    {
        let context = self.context.clone();
        self.context.spawn({
            async move {
                let resolve = context.request_from_shell(PlayOperation::Permissions).await;
                context.update_app(notify(resolve));
            }
        });
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
                _ = context.stream_from_shell(PlayOperation::RunUnit);
                while let Some(resolve) = receiver.next().await {
                    log::info!("core: unit resolved ev: {resolve:?}");
                    context.update_app(notify(resolve))
                }
                log::info!("resolve exited");
            }
        });
    }
}
