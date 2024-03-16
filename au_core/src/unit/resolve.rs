use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use serde::{Deserialize, Serialize};

use ::shared::{FFTData, SnoopsData, UnitResolve};

#[derive(Capability)]
pub struct Resolve<Ev> {
    context: CapabilityContext<UnitResolve, Ev>,
}

impl<Ev> Resolve<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<UnitResolve, Ev>) -> Self {
        Self { context }
    }

    pub fn run(&self, success: bool) {
        self.context.spawn({
            let context = self.context.clone();
            async move { context.notify_shell(UnitResolve::RunUnit(success)).await }
        });
    }

    pub fn create(&self) {
        self.context.spawn({
            let context = self.context.clone();
            async move { context.notify_shell(UnitResolve::Create).await }
        });
    }

    pub fn update(&self, success: bool) {
        self.context.spawn({
            let context = self.context.clone();
            async move { context.notify_shell(UnitResolve::UpdateEV(success)).await }
        });
    }

    pub fn fft(&self, data: FFTData) {
        self.context.spawn({
            let context = self.context.clone();
            async move { context.notify_shell(UnitResolve::FftData(data)).await }
        });
    }

    pub fn snoops(&self, data: SnoopsData) {
        self.context.spawn({
            let context = self.context.clone();
            async move { context.notify_shell(UnitResolve::SnoopsData(data)).await }
        });
    }

    // pub fn run<F>(&self, notify: F)
    // where
    //     F: Fn(UnitResolve) -> Ev + Send + 'static,
    // {
    //     let context
    // }
}
