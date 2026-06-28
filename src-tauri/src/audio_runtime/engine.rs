mod error;
mod input;
mod output;

use std::sync::Arc;

use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, DeviceId, HostId,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tauri::async_runtime;

use crate::{
    audio_runtime::{
        engine::{input::Input, output::Output},
        stream::AudioStreamError,
    },
    dsp::{Analyzer, Synthesizer},
};

use super::stream::{
    create_telemetry_channel, PlaybackQuality, QualityGateManger, TelemetrySender,
};

pub use error::AudioEngineError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct DeviceData {
    pub host_id: String,
    pub device_id: String,
    pub device_name: String,
    pub device_manufacturer: Option<String>,
    pub supports_input: bool,
    pub supports_output: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub enum PlaybackState {
    Playing,
    Paused,
    #[default]
    Stopped,
}

/// CPAL-backed stream engine managing the audio runtime subsystem and device streams.
pub struct AudioEngine {
    state: RwLock<PlaybackState>,
    host: RwLock<cpal::Host>,
    // quality
    manager_thread: RwLock<Option<async_runtime::JoinHandle<()>>>,
    current_quality: Arc<RwLock<PlaybackQuality>>,
    telemetry_sender: TelemetrySender,
    // output
    pub output_stream: RwLock<Option<Output>>,
    // input
    pub input_stream: RwLock<Option<Input>>,
}

impl AudioEngine {
    pub fn new() -> Arc<Self> {
        let (telemetry_sender, telemetry_receiver) = create_telemetry_channel();
        let current_quality = Arc::new(RwLock::new(PlaybackQuality::default()));
        let mut manager = QualityGateManger::new(telemetry_receiver, current_quality.clone());

        let engine = Arc::new(Self {
            state: RwLock::new(PlaybackState::default()),
            manager_thread: RwLock::new(None),
            current_quality,
            telemetry_sender,
            output_stream: RwLock::new(None),
            input_stream: RwLock::new(None),
            host: RwLock::new(cpal::default_host()),
        });

        let manager_thread = async_runtime::spawn({
            let engine = engine.clone();
            async move {
                manager
                    .run(move |new_quality| {
                        if let Err(e) = engine.on_quality_change(new_quality) {
                            log::error!("Failed to handle quality change: {e}");
                            false
                        } else {
                            log::info!("Successfully handled quality change to: {:?}", new_quality);
                            true
                        }
                    })
                    .await;
            }
        });

        *engine.manager_thread.write() = Some(manager_thread);

        engine
    }

    pub fn on_quality_change(
        &self,
        new_quality: PlaybackQuality,
    ) -> Result<(), super::AudioRuntimeError> {
        log::info!("Playback quality changed to: {:?}", new_quality);

        *self.current_quality.write() = new_quality;

        if let Some(input) = self.input_stream.write().as_mut() {
            if input.on_quality_change() {
                self.restart_input_stream()?;
            }
        }

        if let Some(output) = self.output_stream.write().as_mut() {
            if output.on_quality_change() {
                self.restart_output_stream()?;
            }
        }

        Ok(())
    }

    fn device_from_data(host_id: &str, device_id: &str) -> Option<Device> {
        let host_id: HostId = host_id.parse().ok()?;
        let device_id: DeviceId = DeviceId::new(host_id, device_id);
        cpal::host_from_id(host_id)
            .ok()
            .and_then(|host| host.device_by_id(&device_id))
    }

    pub fn list_devices(&self) -> Vec<DeviceData> {
        cpal::available_hosts()
            .into_iter()
            .filter_map(|host_id| {
                cpal::host_from_id(host_id)
                    .ok()
                    .and_then(|host| host.devices().ok())
            })
            .flat_map(|devices| {
                devices.filter_map(|d| {
                    d.id()
                        .ok()
                        .and_then(move |id| d.description().ok().map(move |desc| (d, id, desc)))
                })
            })
            .map(|(device, device_id, description)| DeviceData {
                host_id: device_id.host().to_string(),
                device_id: device_id.id().to_string(),
                device_name: description.name().to_string(),
                device_manufacturer: description.manufacturer().map(|m| m.to_string()),
                supports_input: device.supports_input(),
                supports_output: device.supports_output(),
            })
            .collect()
    }

    pub fn select_input_device(
        &self,
        host_id: String,
        device_id: String,
    ) -> Result<(), super::AudioRuntimeError> {
        let device = Self::device_from_data(&host_id, &device_id)
            .ok_or(error::AudioEngineError::DeviceUnavailable)?;

        if !device.supports_input() {
            return Err(error::AudioEngineError::InputNotSupportedBySelectedInputDevice.into());
        }

        if let Some(input) = self.input_stream.write().as_mut() {
            input.device = Some(device);
        }
        self.restart_input_stream()?;

        Ok(())
    }

    pub fn select_output_device(
        &self,
        host_id: String,
        device_id: String,
    ) -> Result<(), super::AudioRuntimeError> {
        let device = Self::device_from_data(&host_id, &device_id)
            .ok_or(error::AudioEngineError::DeviceUnavailable)?;

        if !device.supports_output() {
            return Err(error::AudioEngineError::OutputNotSupportedBySelectedOutputDevice.into());
        }

        if let Some(output) = self.output_stream.write().as_mut() {
            output.device = Some(device);
        }
        self.restart_output_stream()?;

        Ok(())
    }

    fn start_input_stream(&self, net: Arc<Analyzer>) -> Result<(), super::AudioRuntimeError> {
        if self.input_stream.read().is_some() {
            log::debug!("Input stream already running, skipping start");
            return Ok(());
        }

        let host = cpal::host_from_id(self.host.read().id()).unwrap();
        let input = Input::new(host, self.current_quality.clone(), net);

        *self.input_stream.write() = Some(input);

        let mut input_lock = self.input_stream.write();

        input_lock.as_mut().unwrap().start()?;

        Ok(())
    }

    fn stop_input_stream(&self) -> Result<Option<Input>, super::AudioRuntimeError> {
        if let Some(mut input) = self.input_stream.write().take() {
            input.stop()?;
            Ok(Some(input))
        } else {
            log::error!("No input stream available to stop");
            Ok(None)
        }
    }

    fn restart_input_stream(&self) -> Result<(), super::AudioRuntimeError> {
        let Input { net, .. } = self
            .stop_input_stream()?
            .ok_or(AudioStreamError::BackendMissing)?;
        self.start_input_stream(net)?;
        Ok(())
    }

    fn start_output_stream(&self, net: Arc<Synthesizer>) -> Result<(), super::AudioRuntimeError> {
        if self.output_stream.read().is_some() {
            log::debug!("Output stream already running, skipping start");
            return Ok(());
        }

        let host = cpal::host_from_id(self.host.read().id()).unwrap();
        let output = Output::new(
            host,
            self.current_quality.clone(),
            Some(self.telemetry_sender.clone()),
            self.input_stream
                .read()
                .as_ref()
                .and_then(|input| input.buffer.clone()),
            net,
        );

        *self.output_stream.write() = Some(output);

        let mut output_lock = self.output_stream.write();

        output_lock.as_mut().unwrap().start()?;

        Ok(())
    }

    fn stop_output_stream(&self) -> Result<Option<Output>, super::AudioRuntimeError> {
        if let Some(mut output) = self.output_stream.write().take() {
            output.stop()?;
            Ok(Some(output))
        } else {
            log::error!("No output stream available to stop");
            Ok(None)
        }
    }

    fn restart_output_stream(&self) -> Result<(), super::AudioRuntimeError> {
        let Output { net, .. } = self
            .stop_output_stream()?
            .ok_or(AudioStreamError::BackendMissing)?;
        self.start_output_stream(net)?;
        Ok(())
    }

    pub fn start(
        &self,
        input: Option<Arc<Analyzer>>,
        output: Option<Arc<Synthesizer>>,
    ) -> Result<(), super::AudioRuntimeError> {
        super::audio_session::ensure_configured()?;

        if let Some(input_net) = input {
            self.start_input_stream(input_net)?;
            log::info!("Input stream started");
        }

        if let Some(output_net) = output {
            self.start_output_stream(output_net)?;
            log::info!("Output stream started");
        }

        self.state.write().clone_from(&PlaybackState::Playing);

        Ok(())
    }

    pub fn restart_with(
        &self,
        input: Option<Arc<Analyzer>>,
        output: Option<Arc<Synthesizer>>,
    ) -> Result<(), super::AudioRuntimeError> {
        self.stop()?;
        self.start(input, output)?;
        Ok(())
    }

    pub fn stop(&self) -> Result<(), super::AudioRuntimeError> {
        if self.input_stream.read().is_some() {
            self.stop_input_stream()?;
        }
        if self.output_stream.read().is_some() {
            self.stop_output_stream()?;
        }

        self.state.write().clone_from(&PlaybackState::Stopped);

        Ok(())
    }

    pub fn pause(&self) -> Result<(), super::AudioRuntimeError> {
        if let Some(output) = self.output_stream.write().as_mut() {
            output.pause()?;
        }

        if let Some(input) = self.input_stream.write().as_mut() {
            input.pause()?;
        }

        self.state.write().clone_from(&PlaybackState::Paused);

        Ok(())
    }

    pub fn resume(&self) -> Result<(), super::AudioRuntimeError> {
        if let Some(output) = self.output_stream.write().as_mut() {
            output.resume()?;
        }

        if let Some(input) = self.input_stream.write().as_mut() {
            input.resume()?;
        }

        self.state.write().clone_from(&PlaybackState::Playing);

        Ok(())
    }

    pub fn playback_state(&self) -> PlaybackState {
        self.state.read().clone()
    }

    pub fn current_quality(&self) -> PlaybackQuality {
        *self.current_quality.read()
    }
}
