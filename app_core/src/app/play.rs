use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use serde::{Deserialize, Serialize};

use crate::tuner::TuningValue;

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
    Config(Config, Vec<Node>, Vec<TuningValue>),
    Input(Vec<Vec<f32>>),
    SendSnoops
}

impl Eq for PlayOperation {}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum PlayOperationOutput {
    Success,
    Failure
}

impl Eq for PlayOperationOutput {}
impl Operation for PlayOperation {
    type Output = PlayOperationOutput;
}

impl Operation for PlayOperationOutput {
    type Output = ();
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum CaptureOutput {
    CaptureFFT(Vec<(f32, f32)>),
    CaptureData(Vec<f32>),
    CaptureNodesData(Vec<(usize, Vec<f32>)>)
}

impl Eq for CaptureOutput {}

impl Operation for CaptureOutput {
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

    pub fn configure<F>(&self, config: &Config, nodes: &[Node], tuning: &[TuningValue], f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();
        let config = config.clone();
        let nodes = Vec::from(nodes);
        let tuning = Vec::from(tuning);

        self.context.spawn(async move {
            let done = ctx
                .request_from_shell(PlayOperation::Config(config, nodes, tuning))
                .await;
            ctx.update_app(f(done == PlayOperationOutput::Success));
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
            ctx.update_app(f(playing == PlayOperationOutput::Success));
        })
    }

    pub fn pause<F>(&self, f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            let paused = ctx.request_from_shell(PlayOperation::Suspend).await;
            ctx.update_app(f(paused == PlayOperationOutput::Success));
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
            ctx.update_app(f(done == PlayOperationOutput::Success));
        })
    }
    
    pub fn query_snoops(&self)
    {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            ctx.notify_shell(PlayOperation::SendSnoops).await;
        })
    }

    pub fn permissions<F>(&self, f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            let granted = ctx.request_from_shell(PlayOperation::Permissions).await;
            ctx.update_app(f(granted == PlayOperationOutput::Success));
        })
    }
    pub fn capture_fft<F>(&self, notify: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();
        self.context.spawn(async move {
            let capturing = ctx.request_from_shell(PlayOperation::Capture(true)).await;
            ctx.update_app(notify(capturing == PlayOperationOutput::Success));
        });
    }

    pub fn stop_capture_fft<F>(&self, notify: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static, {
        let ctx = self.context.clone();
        self.context.spawn(async move {
            let stopped = ctx.request_from_shell(PlayOperation::Capture(false)).await;
            ctx.update_app(notify(stopped == PlayOperationOutput::Success));
        })
    }
}
