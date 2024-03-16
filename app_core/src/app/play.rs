use std::sync::Arc;

use ::shared::*;
use anyhow::Result;
use crux_core::{
    capability::{CapabilityContext, Operation},
    Core,
};
use crux_macros::Capability;
use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    SinkExt, StreamExt,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PlayOperation {}

impl Operation for PlayOperation {
    type Output = UnitResolve;
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

    // pub fn run<F>(&self, notify: F)
    // where
    //     F: Fn(UnitResolve) -> Ev + Send + 'static,
    // {
    // }
}
