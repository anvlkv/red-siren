use anyhow::Result;
use app_core::{
    AnimateOperation, AnimateOperationOutput, PermissionsOperation, PermissionsOperationOutput,
    Request,
};
use au_core::Backend;
use cpal::traits::DeviceTrait;
use crux_core::capability::{Capability, CapabilityContext, Operation};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ShellOp {
    Animate(AnimateOperation),
    Permissions(PermissionsOperation),
}

pub enum ShellOutput {
    Animate(AnimateOperationOutput),
    Permissions(PermissionsOperationOutput),
}

impl Into<AnimateOperationOutput> for ShellOutput {
    fn into(self) -> AnimateOperationOutput {
        match self {
            ShellOutput::Animate(op) => op,
            _ => panic!("unexpected output"),
        }
    }
}

impl Into<PermissionsOperationOutput> for ShellOutput {
    fn into(self) -> AnimateOperationOutput {
        match self {
            ShellOutput::Permissions(op) => op,
            _ => panic!("unexpected output"),
        }
    }
}

impl Operation for ShellOp {
    type Output = ShellOutput;
}

#[derive(Capability)]
pub struct ShellCapability<Ev> {
    context: CapabilityContext<ShellOp, Ev>,
}

impl<Ev> ShellCapability<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<ShellOp, Ev>) -> Self {
        Self { context }
    }

    pub fn resolve<Op, F>(&self, op: &Op, on_resolve: F)
    where
        Op: Operation,
        Op::Output: From<ShellOutput>,
        F: Fn(Op::Output) + Send + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();
            let op = op.clone();
            async move {
                let stream = context.stream_from_shell(op);
                while let Some(result) = stream.next().await {
                    on_resolve(result.into())
                }
            }
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_streams(&self, backend: Backend) -> Result<()> {
        let host = cpal::default_host();
        let input = cpal::traits::HostTrait::default_input_device(&host);
        let output = cpal::traits::HostTrait::default_output_device(&host);

        log::info!(
            "using input: {:?}",
            input
                .as_ref()
                .map(|d| cpal::traits::DeviceTrait::name(d).ok())
                .flatten()
        );
        log::info!(
            "using output: {:?}",
            output
                .as_ref()
                .map(|d| cpal::traits::DeviceTrait::name(d).ok())
                .flatten()
        );

        let channels = output
            .map(|o| o.default_output_config().ok().map(|c| c.channels()))
            .flatten()
            .unwrap_or(1);

        let (mut input_cb, mut output_cb) = backend.into_split(channels);

        let in_stream = input
            .map(move |input| {
                if let Ok(config) = cpal::traits::DeviceTrait::default_input_config(&input) {
                    let config: cpal::StreamConfig = config.into();
                    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        input_cb(data);
                    };
                    let stream = cpal::traits::DeviceTrait::build_input_stream(
                        &input,
                        &config,
                        input_data_fn,
                        err_fn,
                        None,
                    )
                    .expect("create stream");
                    Some(stream)
                } else {
                    None
                }
            })
            .flatten();

        let out_stream = output
            .map(move |output| {
                if let Ok(config) = cpal::traits::DeviceTrait::default_output_config(&output) {
                    let config: cpal::StreamConfig = config.into();
                    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        output_cb(data);
                    };

                    let stream = cpal::traits::DeviceTrait::build_output_stream(
                        &output,
                        &config,
                        output_data_fn,
                        err_fn,
                        None,
                    )
                    .expect("create stream");
                    Some(stream)
                } else {
                    None
                }
            })
            .flatten();

        Ok(())
    }
}
