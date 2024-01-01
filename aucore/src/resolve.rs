use crux_core::capability::CapabilityContext;
use crux_macros::Capability;

use shared::play::PlayOperationOutput;

#[derive(Capability)]
pub struct Resolve<Ev> {
    context: CapabilityContext<PlayOperationOutput, Ev>,
}

impl<Ev> Resolve<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<PlayOperationOutput, Ev>) -> Self {
        Self { context }
    }

    pub fn resolve_capture_fft(&self, captured: Vec<(f32, f32)>) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            _ = ctx
                .notify_shell(PlayOperationOutput::CapturedFFT(captured))
                .await;
        })
    }

    pub fn resolve_permission(&self, result: bool) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            _ = ctx
                .notify_shell(PlayOperationOutput::Permission(result))
                .await;
        })
    }

    pub fn resolve_success(&self, success: bool) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            _ = ctx
                .notify_shell(PlayOperationOutput::Success(success))
                .await;
        })
    }

    pub fn resolve_devices(&self, devices: Vec<String>) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            _ = ctx
                .notify_shell(PlayOperationOutput::Devices(devices))
                .await;
        })
    }

    pub fn resolve_none(&self) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            _ = ctx.notify_shell(PlayOperationOutput::None).await;
        })
    }
}
