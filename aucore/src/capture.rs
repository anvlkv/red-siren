use crux_core::capability::CapabilityContext;
use crux_macros::Capability;
use app_core::play::CaptureOutput;


#[derive(Capability)]
pub struct Capture<Ev> {
    context: CapabilityContext<CaptureOutput, Ev>,
}

impl<Ev> Capture<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<CaptureOutput, Ev>) -> Self {
        Self { context }
    }

    pub fn capture_fft(&self, captured: Vec<(f32, f32)>) {
        let ctx = self.context.clone();

        self.context.spawn(async move {
            ctx.notify_shell(CaptureOutput::CaptureFFT(captured)).await;
        })
    }
}
