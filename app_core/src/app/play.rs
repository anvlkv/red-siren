use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::instrument::{Config, Node};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum PlayOperation {
    Permissions,
    InstallAU,
    Suspend,
    Resume,
    Capture(bool),
    QueryInputDevices,
    QueryOutputDevices,
    Config(Config, Vec<Node>),
    Input(Vec<Vec<f32>>),
}

impl Eq for PlayOperation {}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum PlayOperationOutput {
    CapturedFFT(Vec<(f32, f32)>),
    Devices(Vec<String>),
    Success(bool),
    Permission(bool),
    None,
}

impl Eq for PlayOperationOutput {}
impl Operation for PlayOperation {
    type Output = PlayOperationOutput;
}

impl Operation for PlayOperationOutput {
    type Output = ();
}

#[derive(Capability)]
pub struct Play<Ev> {
    context: CapabilityContext<PlayOperation, Ev>,
}

impl<Ev> Play<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<PlayOperation, Ev>) -> Self {
        Self { context }
    }

    pub fn configure<F>(&self, config: &Config, nodes: &[Node], f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();
        let config = config.clone();
        let nodes = Vec::from(nodes);

        self.context.spawn(async move {
            let done = ctx
                .request_from_shell(PlayOperation::Config(config, nodes))
                .await;
            if let PlayOperationOutput::Success(done) = done {
                ctx.update_app(f(done));
            } else {
                log::warn!("play unexpected variant: {done:?}");
            }
        })
    }

    pub fn play<F>(&self, f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            let playing = ctx.request_from_shell(PlayOperation::Resume).await;
            if let PlayOperationOutput::Success(playing) = playing {
                ctx.update_app(f(playing));
            } else {
                log::warn!("play unexpected variant: {playing:?}");
            }
        })
    }

    pub fn pause(&self) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            ctx.notify_shell(PlayOperation::Suspend).await;
        })
    }

    pub fn install_au<F>(&self, f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            let done = ctx.request_from_shell(PlayOperation::InstallAU).await;
            if let PlayOperationOutput::Success(done) = done {
                ctx.update_app(f(done));
            } else {
                log::warn!("install unexpected variant: {done:?}");
            }
        })
    }

    pub fn permissions<F>(&self, f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            let done = ctx.request_from_shell(PlayOperation::Permissions).await;
            if let PlayOperationOutput::Permission(done) = done {
                ctx.update_app(f(done));
            } else {
                log::warn!("permissions unexpected variant: {done:?}");
            }
        })
    }
    pub fn capture_fft<F>(&self, notify: F)
    where
        Ev: 'static,
        F: Fn(Vec<(f32, f32)>) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();
        self.context.spawn({
            async move {
                let mut stream = ctx.stream_from_shell(PlayOperation::Capture(true));
                while let Some(response) = stream.next().await {
                    if let PlayOperationOutput::CapturedFFT(data) = response {
                        ctx.update_app(notify(data));
                    } else {
                        break;
                    }
                }

                log::info!("capture exited");
            }
        });
    }

    pub fn stop_capture_fft<F>(&self, f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();
        self.context.spawn(async move {
            let stopped = ctx.request_from_shell(PlayOperation::Capture(false)).await;
            if let PlayOperationOutput::Success(stopped) = stopped {
                ctx.update_app(f(stopped));
            } else {
                log::warn!("pause unexpected variant: {stopped:?}");
            }
        })
    }
}
