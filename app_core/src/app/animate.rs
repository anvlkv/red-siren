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

    pub fn start<F>(&self, notify: F, label: &str)
    where
        F: Fn(f64) -> Ev + Send + 'static,
    {
        log::info!("starting {label} animation");

        let context = self.context.clone();

        let label = label.to_string();
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

    pub fn animate_reception<F, T, R>(&self, notify: F, mut cons: R, label: &str)
    where
        F: Fn(T) -> Ev + Send + 'static,
        R: FnMut() -> Option<T> + Send + 'static,
        T: Send + 'static,
    {
        let context = self.context.clone();

        let label = label.to_string();
        self.context.spawn({
            async move {
                let mut stream = context.stream_from_shell(AnimateOperation::Start);
                log::info!("core: starting animate reception {label}");

                while let Some(response) = stream.next().await {
                    log::info!("receive ts stream");

                    if let AnimateOperationOutput::Timestamp(_) = response {
                        match cons() {
                            Some(d) => {
                                context.update_app(notify(d));
                            }
                            None => {
                                log::debug!("no data in receiver for {label}");
                            }
                        }
                    } else {
                        break;
                    }
                }

                log::info!("animate reception exited {label}");
            }
        });
    }

    pub fn stop<F>(&self, notify: F)
    where
        F: Fn() -> Ev + Send + 'static,
    {
        log::debug!("stopping animation");

        let context = self.context.clone();

        self.context.spawn({
            async move {
                let done = context.request_from_shell(AnimateOperation::Stop).await;
                if done == AnimateOperationOutput::Done {
                    context.update_app(notify());

                    log::info!("animation stopped");
                } else {
                    panic!("unexpected response")
                }
            }
        });
    }
}
