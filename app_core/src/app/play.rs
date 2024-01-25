use std::{
    error::Error,
    sync::{mpsc::sync_channel, Arc, Mutex},
};

use au_core::{Unit, UnitEV};
use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use hecs::Entity;
use serde::{Deserialize, Serialize};

// static UNIT_INSTANCE: Lazy<> = Lazy::new(|| Default::default());

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PlayOperation {
    Permissions,
    InstallAU,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PlayOperationOutput {
    Success,
    Failure,
    PermanentFailure,
}

impl Operation for PlayOperation {
    type Output = PlayOperationOutput;
}

#[derive(Capability)]
pub struct Play<Ev> {
    context: CapabilityContext<PlayOperation, Ev>,
    unit: Arc<Mutex<Option<Unit>>>,
}

impl<Ev> Play<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<PlayOperation, Ev>) -> Self {
        Self {
            context,
            unit: Default::default(),
        }
    }

    pub fn install<F>(&self, notify: F)
    where
        F: Fn(PlayOperationOutput) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();
        let mtx = self.unit.clone();

        self.context.spawn(async move {
            let unit = Unit::new();

            let mut u_mtx = mtx.lock().expect("lock unit");
            _ = u_mtx.insert(unit);
            ctx.update_app(notify(PlayOperationOutput::Success));
        });
    }

    pub fn run_unit<F, FF, FS>(&self, notify: F, fft_ev: FF, snoop_ev: FS)
    where
        F: Fn(PlayOperationOutput) -> Ev + Send + 'static,
        FF: Fn(Vec<(f32, f32)>) -> Ev + Send + 'static,
        FS: Fn(Vec<(Entity, Vec<f32>)>) -> Ev + Send + 'static,
    {
        let ctx = self.context.clone();
        let mtx = self.unit.clone();

        let (fft_sender, fft_receiver) = sync_channel(32);
        let (snoop_sender, snoop_receiver) = sync_channel(64);

        self.context.spawn(async move {
            match mtx.lock() {
                Ok(mut unit) => {
                    if let Some(unit) = unit.as_mut() {
                        match unit.run(fft_sender, snoop_sender) {
                            Ok(_) => {
                                ctx.update_app(notify(PlayOperationOutput::Success));
                            }
                            Err(e) => {
                                log::error!("unit run error: {:?}", e);
                                ctx.update_app(notify(PlayOperationOutput::PermanentFailure));
                            }
                        }
                    } else {
                        log::error!("no unit");
                        ctx.update_app(notify(PlayOperationOutput::Failure));
                    }
                }
                Err(r) => {
                    log::error!("mutex poison");
                    mtx.clear_poison();
                    ctx.update_app(notify(PlayOperationOutput::Failure));
                }
            }
        });

        let ctx = self.context.clone();
        self.context.spawn(async move {
            let mut it = fft_receiver.into_iter();
            while let Some(d) = it.next() {
                ctx.update_app(fft_ev(d));
            }
        });

        let ctx = self.context.clone();
        self.context.spawn(async move {
            let mut it = snoop_receiver.into_iter();
            while let Some(d) = it.next() {
                ctx.update_app(snoop_ev(d));
            }
        });
    }

    
}
