use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use serde::{Deserialize, Serialize};

use super::instrument::{Config, Node};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum PlayOperation {
    Permissions,
    InstallAU,
    Suspend,
    Resume,
    QueryInputDevices,
    QueryOutputDevices,
    Config(Config, Vec<Node>),
    Input(Vec<Vec<f32>>),
}

impl Eq for PlayOperation {}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PlayOperationOutput {
    Devices(Vec<String>),
    Success(bool),
    Permission(bool),
    None,
}

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

    pub fn pause<F>(&self, f: F)
    where
        Ev: 'static,
        F: Fn(bool) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            let paused = ctx.request_from_shell(PlayOperation::Suspend).await;
            if let PlayOperationOutput::Success(paused) = paused {
                ctx.update_app(f(paused));
            } else {
                log::warn!("pause unexpected variant: {paused:?}");
            }
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
}
