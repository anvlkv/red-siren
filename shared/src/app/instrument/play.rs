use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Data, FromSample, Sample, SampleFormat, InputCallbackInfo};
use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::sync::{Arc, Mutex, mpsc::channel};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PlayOperation {
    Start,
    Suspend,
    Resume,
    QueryDevices,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PlayOperationOutput {
    Devices(Vec<String>, Vec<String>),

}

impl Operation for PlayOperation {
    type Output = PlayOperationOutput;
}



#[derive(Capability)]
pub struct Play<Ev> {
    context: CapabilityContext<PlayOperation, Ev>,
    host: cpal::Host,
}

impl<Ev> Play<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<PlayOperation, Ev>) -> Self {
        Self {
            context,
            host: cpal::default_host(),
        }
    }

    pub fn play(&self, play_sys: Arc<Mutex<super::System>>) {
        let ctx = self.context.clone();
        self.context.spawn(async move {
          let result = {
            
            
          };
        })
    }

    pub fn suspend(&self) {
        let ctx = self.context.clone();
        self.context.spawn(async move {})
    }

    pub fn query_devices<F>(&self, ev: F)
    where
        Event: 'static,
        F: Fn((Vec<String>, Vec<String>)) -> Event + Send + 'static,
    {
        let ctx = self.context.clone();
        let host_id = self.host.id();

        self.context.spawn(async move {
            let result = {
                let host = cpal::host_from_id(host_id)?;

                let inputs = host
                    .input_devices()?
                    .filter_map(|d| d.name().ok())
                    .collect::<Vec<_>>();
                let outputs = host
                    .output_devices()?
                    .filter_map(|d| d.name().ok())
                    .collect::<Vec<_>>();

                Ok((inputs, outputs))
            };

            ctx.update_app(ev(result))
        })
    }

    pub fn configure_input(&self, input: &str, input_ev: F)where
    Event: 'static,
    F: Fn() -> Event + Send + 'static,  {
        let ctx = self.context.clone();
        let host_id = self.host.id();
        self.context.spawn(async move {
            let result = {
                let host = cpal::host_from_id(host_id)?;

                let input_device = host
                    .input_devices()?
                    .try_find(|d| Ok(d.name()?.as_str() == input))?
                    .ok_or("Selected device not found")?;
                let mut input_configs_range =
                    input_device
                        .supported_input_configs()?
                        .filter(|c| match c.sample_format() {
                            SampleFormat::F32
                            | SampleFormat::F64
                            | SampleFormat::I16
                            | SampleFormat::U16 => true,
                            _ => false,
                        });
                let input_config = input_configs_range
                    .max_by(|x, y| x.cmp_default_heuristics(y))
                    .ok_or("No input config found")?;

                    let sample_format = input_config.sample_format();
                  let config:  = input_config.into();

                // fn data_callback<T: Sample>(data: &[T], info: &InputCallbackInfo) {

                // }

                let stream = match sample_format {
                  SampleFormat::F32 => input_device.build_input_stream(&config, |d, _| ctx.update_app(event), error_callback, None)?,
                  SampleFormat::F64 => input_device.build_input_stream(&config, data_callback, error_callback, None)?,
                  SampleFormat::I16 => input_device.build_input_stream(&config, data_callback, error_callback, None)?,
                  SampleFormat::U16 => input_device.build_input_stream(&config, data_callback, error_callback, None)?,
                  _ => return anyhow!("Unsupported format")
                };


                Ok(())
            };

            // ctx.update_app(ev(result))
        })
    }

    pub fn configure_output(&self, input: &str) {}

    fn create(&self) -> Result<()> {
        // let host = cpal::default_host();

        let output_device = host.default_output_device().ok_or("no output device")?;
        let input_device = host
            .default_input_device()
            .ok_or("no input device available")?;

        let mut output_configs_range =
            output_device
                .supported_output_configs()?
                .filter(|c| match c.sample_format() {
                    SampleFormat::F32
                    | SampleFormat::F64
                    | SampleFormat::I16
                    | SampleFormat::U16 => true,
                    _ => false,
                });
        let mut input_configs_range =
            input_device
                .supported_input_configs()?
                .filter(|c| match c.sample_format() {
                    SampleFormat::F32
                    | SampleFormat::F64
                    | SampleFormat::I16
                    | SampleFormat::U16 => true,
                    _ => false,
                });

        let output_config = output_configs_range
            .max_by(|x, y| x.cmp_default_heuristics(y))
            .ok_or("No output config found")?;

        let input_config = input_configs_range
            .max_by(|x, y| x.cmp_default_heuristics(y))
            .ok_or("No input config found")?;

        Ok(())
    }
}
