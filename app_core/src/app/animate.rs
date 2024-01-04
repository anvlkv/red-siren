use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum AnimateOperation {
    Start,
    Stop,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum AnimateOperationOutput {
    Timestamp(f64),
    Done,
}

impl Eq for AnimateOperationOutput {}

impl Operation for AnimateOperation {
    type Output = AnimateOperationOutput;
}

#[derive(Capability)]
pub struct Animate<Ev> {
    context: CapabilityContext<AnimateOperation, Ev>,
}

impl<Ev> Animate<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<AnimateOperation, Ev>) -> Self {
        Self { context }
    }

    pub fn start<F>(&self, notify: F, label: String)
    where
        F: Fn(f64) -> Ev + Send + 'static,
    {
        log::debug!("starting animation");

        let context = self.context.clone();

        self.context.spawn({
            async move {
                let mut stream = context.stream_from_shell(AnimateOperation::Start);

                while let Some(response) = stream.next().await {
                    if let AnimateOperationOutput::Timestamp(ts) = response {
                        context.update_app(notify(ts));
                    } else {
                        break;
                    }
                }

                log::info!("animation {label} exited")
            }
        });
    }

    pub fn stop(&self) {
        log::debug!("stopping animation");

        let context = self.context.clone();
        
        self.context.spawn({
            async move {
                _ = context.notify_shell(AnimateOperation::Stop).await;
            }
        });
    }
}
