use app_core::play::PlayOperationOutput;
use crux_core::capability::CapabilityContext;
use crux_macros::Capability;

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

    pub fn resolve_success(&self, success: bool) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            _ = ctx
                .notify_shell(if success {
                    PlayOperationOutput::Success
                } else {
                    PlayOperationOutput::Failure
                })
                .await;
        })
    }
}
