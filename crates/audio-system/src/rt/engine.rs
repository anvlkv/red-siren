use std::{
    f32,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use common::{
    device::DeviceData,
    error::{AppError, AudioDeviceError, AudioStreamError, Result},
    playback_quality::PlaybackQuality,
};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, DeviceId, HostId, StreamConfig, SupportedStreamConfig,
};
use fundsp::prelude::*;
use fundsp::{thingbuf::ThingBuf, typenum::Unsigned};
use parking_lot::RwLock;
use u_num_it::u_num_it;

use crate::{
    quality::PlaybackQualityGate,
    rt::{
        rt_subsystem::RuntimeSubsystem,
        stream::{playback_callback, spawn_owned_input_stream, PlaybackCallbackConfig},
        telemetry::TelemetrySender,
        ExcitementSource,
    },
};

use super::stream::{spawn_owned_output_stream, Control};

const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;

const INPUT_BUFFER_DURATION_MS: f64 = 15_f64;

/// CPAL-backed stream engine managing the audio runtime subsystem and device streams.
pub struct Engine {
    runtime: RwLock<Option<RuntimeSubsystem>>,
    // quality
    telemetry_buffer: RwLock<TelemetrySender>,
    quality_gate: Arc<RwLock<PlaybackQualityGate>>,
    // output
    output_device: RwLock<Option<Device>>,
    output_config: RwLock<Option<SupportedStreamConfig>>,
    output_thread: RwLock<Option<thread::JoinHandle<()>>>,
    output_control: RwLock<Option<Sender<Control>>>,
    // input
    input_device: RwLock<Option<Device>>,
    input_config: RwLock<Option<SupportedStreamConfig>>,
    input_thread: RwLock<Option<thread::JoinHandle<()>>>,
    input_control: RwLock<Option<Sender<Control>>>,
    input_buffer: RwLock<Option<Arc<ThingBuf<f32>>>>,
    host: Option<cpal::Host>,
}

impl Engine {
    pub fn new(
        telemetry: TelemetrySender,
        output_device: Option<DeviceData>,
        input_device: Option<DeviceData>,
    ) -> Self {
        Self::new_with_host(telemetry, output_device, input_device, None)
    }

    pub fn new_with_host(
        telemetry: TelemetrySender,
        output_device: Option<DeviceData>,
        input_device: Option<DeviceData>,
        host: Option<cpal::Host>,
    ) -> Self {
        let input_device = input_device
            .and_then(|d| Self::device_from_data_with_host(&host, d))
            .or_else(|| Self::default_input_device_with_host(&host));
        let output_device = output_device
            .and_then(|d| Self::device_from_data_with_host(&host, d))
            .or_else(|| Self::default_output_device_with_host(&host));

        let output_config = output_device
            .as_ref()
            .and_then(|d| d.default_output_config().ok());
        let quality_gate = output_config
            .as_ref()
            .cloned()
            .map(PlaybackQualityGate::from)
            .unwrap_or_default();

        Self {
            runtime: RwLock::new(None),
            telemetry_buffer: RwLock::new(telemetry),
            quality_gate: Arc::new(RwLock::new(quality_gate)),

            output_device: RwLock::new(output_device),
            output_config: RwLock::new(output_config),
            output_thread: RwLock::new(None),
            output_control: RwLock::new(None),
            input_config: RwLock::new(
                input_device
                    .as_ref()
                    .and_then(|d| d.default_input_config().ok()),
            ),
            input_device: RwLock::new(input_device),
            input_thread: RwLock::new(None),
            input_control: RwLock::new(None),
            input_buffer: RwLock::new(None),
            host,
        }
    }

    fn device_from_data_with_host(host: &Option<cpal::Host>, data: DeviceData) -> Option<Device> {
        let host_id: HostId = data.host_id.parse().ok()?;
        let device_id: DeviceId = DeviceId(host_id, data.device_id);
        if let Some(h) = host {
            if h.id() == host_id {
                return h.device_by_id(&device_id);
            }
        }
        cpal::host_from_id(host_id)
            .ok()
            .and_then(|host| host.device_by_id(&device_id))
    }

    pub fn list_devices(&self) -> Vec<DeviceData> {
        if let Some(host) = self.host.as_ref() {
            return host
                .devices()
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|d| {
                    d.id()
                        .ok()
                        .and_then(move |id| d.description().ok().map(move |desc| (d, id, desc)))
                })
                .map(|(device, device_id, description)| DeviceData {
                    host_id: device_id.0.to_string(),
                    device_id: device_id.1,
                    device_name: description.name().to_string(),
                    device_manufacturer: description.manufacturer().map(|m| m.to_string()),
                    supports_input: device.supports_input(),
                    supports_output: device.supports_output(),
                })
                .collect();
        }

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
                host_id: device_id.0.to_string(),
                device_id: device_id.1,
                device_name: description.name().to_string(),
                device_manufacturer: description.manufacturer().map(|m| m.to_string()),
                supports_input: device.supports_input(),
                supports_output: device.supports_output(),
            })
            .collect()
    }

    pub fn select_input_device(&self, host_id: String, device_id: String) -> Result<()> {
        let host_id: HostId = host_id.parse().map_err(|_| {
            <AudioDeviceError as Into<AppError>>::into(AudioDeviceError::HostUnavailable)
        })?;
        let device_id: DeviceId = DeviceId(host_id, device_id);
        let device = if let Some(host) = self.host.as_ref().filter(|h| h.id() == host_id) {
            host.device_by_id(&device_id)
        } else {
            cpal::host_from_id(host_id)
                .map_err(|_| {
                    <AudioDeviceError as Into<AppError>>::into(AudioDeviceError::HostUnavailable)
                })?
                .device_by_id(&device_id)
        }
        .ok_or(<AudioDeviceError as Into<AppError>>::into(
            AudioDeviceError::InvalidDeviceId,
        ))?;
        if !device.supports_input() {
            return Err(<AudioDeviceError as Into<AppError>>::into(
                AudioDeviceError::NoSupportForInput,
            ));
        }

        let _prev_device = self.input_device.write().replace(device);

        self.restart_input_stream()?;

        Ok(())
    }

    pub fn select_output_device(&self, host_id: String, device_id: String) -> Result<()> {
        let host_id: HostId = host_id.parse().map_err(|_| {
            <AudioDeviceError as Into<AppError>>::into(AudioDeviceError::HostUnavailable)
        })?;
        let device_id: DeviceId = DeviceId(host_id, device_id);
        let device = if let Some(host) = self.host.as_ref().filter(|h| h.id() == host_id) {
            host.device_by_id(&device_id)
        } else {
            cpal::host_from_id(host_id)
                .map_err(|_| {
                    <AudioDeviceError as Into<AppError>>::into(AudioDeviceError::HostUnavailable)
                })?
                .device_by_id(&device_id)
        }
        .ok_or(<AudioDeviceError as Into<AppError>>::into(
            AudioDeviceError::InvalidDeviceId,
        ))?;
        if !device.supports_output() {
            return Err(<AudioDeviceError as Into<AppError>>::into(
                AudioDeviceError::NoSupportForOutput,
            ));
        }
        let _prev_device = self.output_device.write().replace(device);

        self.restart_output_stream()?;

        Ok(())
    }

    fn input_device(&self) -> Option<Device> {
        self.input_device
            .read()
            .clone()
            .or_else(|| Self::default_input_device_with_host(&self.host))
    }

    fn default_input_device_with_host(host: &Option<cpal::Host>) -> Option<Device> {
        if let Some(h) = host {
            h.default_input_device()
        } else {
            cpal::default_host().default_input_device()
        }
    }

    fn input_cfg(&self) -> Option<SupportedStreamConfig> {
        self.input_config.read().clone().or_else(|| {
            self.input_device()
                .and_then(|d| {
                    self.quality_gate
                        .read()
                        .select_input_config(&d)
                        .or_else(|| d.default_input_config().ok())
                })
                .or_else(|| self.default_input_cfg())
        })
    }

    fn default_input_cfg(&self) -> Option<SupportedStreamConfig> {
        Self::default_input_device_with_host(&self.host).and_then(|d| d.default_input_config().ok())
    }

    fn start_input_stream(&self) -> Result<()> {
        if self.input_thread.read().is_some() {
            return Ok(());
        }

        if let Some((device, default_cfg)) = self.input_device().zip(self.input_cfg()) {
            let stream_cfg: StreamConfig = default_cfg.clone().into();
            let input_buffer = Arc::new(ThingBuf::<f32>::new(
                ((stream_cfg.sample_rate as f64 / 1000_f64) * INPUT_BUFFER_DURATION_MS).round()
                    as usize,
            ));
            *self.input_buffer.write() = Some(input_buffer.clone());

            let (control, handle) =
                spawn_owned_input_stream(device, default_cfg, stream_cfg, move || {
                    Box::new(move |&sample| {
                        if let Err(e) = input_buffer.push(sample) {
                            _ = input_buffer.pop();
                            input_buffer.push(e.into_inner()).unwrap();
                        }
                    })
                })?;

            *self.input_thread.write() = Some(handle);
            *self.input_control.write() = Some(control);

            Ok(())
        } else {
            Err(AppError::AudioDevice(AudioDeviceError::NoSupportForInput))
        }
    }

    fn stop_input_stream(&self) -> Result<()> {
        if let Some(((bf, control), handle)) = self
            .input_buffer
            .write()
            .take()
            .zip(self.input_control.write().take())
            .zip(self.input_thread.write().take())
        {
            let (ack_tx, ack_rx) = mpsc::channel();

            control
                .send(Control::Shutdown(ack_tx))
                .map_err(|e| AudioStreamError::ControlChannelSend { op: e.to_string() })?;

            match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(Ok(_)) => {
                    handle.join().map_err(|_| AudioStreamError::ThreadJoin {
                        op: "join input thread after successful shutdown".to_string(),
                    })?;
                    Ok(())
                }
                Ok(Err(e)) => {
                    handle.join().map_err(|_| AudioStreamError::ThreadJoin {
                        op: format!("join input thread after error: {e}"),
                    })?;
                    log::error!("Stopping input stream failed to invoke control: {e}");
                    Ok(())
                }
                Err(e) => {
                    *self.input_thread.write() = Some(handle);
                    *self.input_control.write() = Some(control);
                    *self.input_buffer.write() = Some(bf);

                    Err(AppError::AudioStream(AudioStreamError::ControlTimeout {
                        op: format!("stopping input stream: {e}"),
                    }))
                }
            }
        } else {
            Err(AppError::AudioStream(AudioStreamError::BackendMissing))
        }
    }

    fn restart_input_stream(&self) -> Result<()> {
        self.stop_input_stream().or_else(|e| {
            if matches!(e, AppError::AudioStream(AudioStreamError::BackendMissing)) {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        self.start_input_stream()?;
        Ok(())
    }

    fn output_device(&self) -> Option<Device> {
        self.output_device
            .read()
            .clone()
            .or_else(|| Self::default_output_device_with_host(&self.host))
    }

    fn default_output_device_with_host(host: &Option<cpal::Host>) -> Option<Device> {
        if let Some(h) = host {
            h.default_output_device()
        } else {
            cpal::default_host().default_output_device()
        }
    }

    fn output_cfg(&self) -> Option<SupportedStreamConfig> {
        self.output_config.read().clone().or_else(|| {
            self.output_device()
                .and_then(|d| {
                    self.quality_gate
                        .read()
                        .select_output_config(&d)
                        .or_else(|| d.default_output_config().ok())
                })
                .or_else(|| self.default_output_cfg())
        })
    }

    fn default_output_cfg(&self) -> Option<SupportedStreamConfig> {
        Self::default_output_device_with_host(&self.host)
            .and_then(|d| d.default_output_config().ok())
    }

    fn start_output_stream(&self) -> Result<()> {
        if self.output_thread.read().is_some() {
            return Ok(());
        }

        if let Some((device, default_cfg)) = self.output_device().zip(self.output_cfg()) {
            let stream_cfg: StreamConfig = default_cfg.clone().into();

            let input_buffer = self.input_buffer.read().clone();
            let sample_rate = stream_cfg.sample_rate;
            let num_channels = std::cmp::Ord::min(stream_cfg.channels as usize, 8);

            let (buffer_target_frames, sample_type, quality) = {
                let qg = self.quality_gate.read();
                (
                    qg.buffer_size(None) as usize,
                    qg.sample_type(),
                    self.quality_gate.clone(),
                )
            };
            let telemetry = self.telemetry_buffer.read().clone();
            let backend = {
                let sys = RuntimeSubsystem::new(sample_type, num_channels, None);
                sys.set_sample_rate(sample_rate as f64);
                let backend = sys.backend();
                *self.runtime.write() = Some(sys);
                backend
            };

            let (control, handle) = spawn_owned_output_stream(
                device,
                default_cfg,
                stream_cfg,
                num_channels,
                move || {
                    let cb_cfg = PlaybackCallbackConfig {
                        input_buffer,
                        sample_rate,
                        buffer_target_frames,
                        sample_type,
                        quality,
                        telemetry,
                    };

                    u_num_it!(
                        1..=8,
                        match num_channels {
                            U => {
                                const N: usize = NumType::USIZE;
                                playback_callback::<N>(backend, cb_cfg)
                            }
                        }
                    )
                },
            )?;

            *self.output_thread.write() = Some(handle);
            *self.output_control.write() = Some(control);

            if let Some(rt) = self.runtime.read().as_ref() {
                rt.fade_in();
            }

            Ok(())
        } else {
            Err(AppError::AudioDevice(AudioDeviceError::NoSupportForOutput))
        }
    }

    fn stop_output_stream(&self) -> Result<()> {
        if let Some(rt) = self.runtime.read().as_ref() {
            rt.fade_out();
        }

        if let Some((control, handle)) = self
            .output_control
            .write()
            .take()
            .zip(self.output_thread.write().take())
        {
            let (ack_tx, ack_rx) = mpsc::channel();

            control
                .send(Control::Shutdown(ack_tx))
                .map_err(|e| AudioStreamError::ControlChannelSend { op: e.to_string() })?;

            match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(Ok(_)) => {
                    handle.join().map_err(|_| AudioStreamError::ThreadJoin {
                        op: "join output thread after successful shutdown".to_string(),
                    })?;
                    Ok(())
                }
                Ok(Err(e)) => {
                    handle.join().map_err(|_| AudioStreamError::ThreadJoin {
                        op: format!("join output thread after error: {e}"),
                    })?;
                    log::error!("Stopping output stream failed to invoke control: {e}");
                    Ok(())
                }
                Err(e) => {
                    *self.output_thread.write() = Some(handle);
                    *self.output_control.write() = Some(control);

                    Err(AppError::AudioStream(AudioStreamError::ControlTimeout {
                        op: format!("stopping output stream: {e}"),
                    }))
                }
            }
        } else {
            Err(AppError::AudioStream(AudioStreamError::BackendMissing))
        }
    }

    fn restart_output_stream(&self) -> Result<()> {
        self.stop_output_stream().or_else(|e| {
            if matches!(e, AppError::AudioStream(AudioStreamError::BackendMissing)) {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        self.start_output_stream()?;
        Ok(())
    }

    pub fn start(&self, source: ExcitementSource) -> common::error::Result<()> {
        super::audio_session::ensure_configured()?;

        if matches!(source, ExcitementSource::Mic) {
            self.start_input_stream()?;
        }

        self.start_output_stream()?;

        Ok(())
    }

    pub fn stop(&self) -> common::error::Result<()> {
        self.stop_input_stream().or_else(|e| {
            if matches!(e, AppError::AudioStream(AudioStreamError::BackendMissing)) {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        self.stop_output_stream()?;
        Ok(())
    }

    pub fn pause(&self) -> common::error::Result<()> {
        if let Some(rt) = self.runtime.read().as_ref() {
            rt.fade_out();
        }

        {
            let output_control = self.output_control.read();
            let Some(cx) = output_control.as_ref() else {
                return Err(AppError::AudioStream(AudioStreamError::BackendMissing));
            };
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Pause(ack_tx)).is_err() {
                return Err(AppError::AudioStream(
                    AudioStreamError::ControlChannelSend {
                        op: "pause output".to_string(),
                    },
                ));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::AudioStream(match e {
                    mpsc::RecvTimeoutError::Timeout => AudioStreamError::ControlTimeout {
                        op: "pause output".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => AudioStreamError::ControlChannelSend {
                        op: "pause output (ack channel disconnected)".to_string(),
                    },
                }));
            }
        }

        if let Some(cx) = self.input_control.read().as_ref() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Pause(ack_tx)).is_err() {
                return Err(AppError::AudioStream(
                    AudioStreamError::ControlChannelSend {
                        op: "pause input".to_string(),
                    },
                ));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::AudioStream(match e {
                    mpsc::RecvTimeoutError::Timeout => AudioStreamError::ControlTimeout {
                        op: "pause input".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => AudioStreamError::ControlChannelSend {
                        op: "pause input (ack channel disconnected)".to_string(),
                    },
                }));
            }
        }

        Ok(())
    }

    pub fn resume(&self) -> common::error::Result<()> {
        if let Some(cx) = self.input_control.read().as_ref() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Resume(ack_tx)).is_err() {
                return Err(AppError::AudioStream(
                    AudioStreamError::ControlChannelSend {
                        op: "resume input".to_string(),
                    },
                ));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::AudioStream(match e {
                    mpsc::RecvTimeoutError::Timeout => AudioStreamError::ControlTimeout {
                        op: "resume input".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => AudioStreamError::ControlChannelSend {
                        op: "resume input (ack channel disconnected)".to_string(),
                    },
                }));
            }
        }

        {
            let output_control = self.output_control.read();
            let Some(cx) = output_control.as_ref() else {
                return Err(AppError::AudioStream(AudioStreamError::BackendMissing));
            };
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Resume(ack_tx)).is_err() {
                return Err(AppError::AudioStream(
                    AudioStreamError::ControlChannelSend {
                        op: "resume output".to_string(),
                    },
                ));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::AudioStream(match e {
                    mpsc::RecvTimeoutError::Timeout => AudioStreamError::ControlTimeout {
                        op: "resume output".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => AudioStreamError::ControlChannelSend {
                        op: "resume output (ack channel disconnected)".to_string(),
                    },
                }));
            }
        }

        if let Some(rt) = self.runtime.read().as_ref() {
            rt.fade_in();
        }

        Ok(())
    }

    pub fn on_excitement_source_changed(
        &self,
        source: ExcitementSource,
    ) -> common::error::Result<()> {
        if matches!(source, ExcitementSource::Mic) && self.input_thread.read().is_none() {
            self.start_input_stream()?;
        } else if matches!(source, ExcitementSource::Entropy) && self.input_thread.read().is_some()
        {
            self.stop_input_stream()?;
        }
        Ok(())
    }

    pub fn on_quality_change(&self, qg: PlaybackQuality) -> common::error::Result<()> {
        let prev_gate = *self.quality_gate.read();
        let next_gate = PlaybackQualityGate::from(qg);
        *self.quality_gate.write() = next_gate;

        if prev_gate.sample_type() != next_gate.sample_type() {
            let sample_type = next_gate.sample_type();
            self.runtime
                .write()
                .as_mut()
                .map(|rt| rt.set_sample_type(sample_type));
        }

        // Compare against the device's actual configured sample rate, not the ideal target.
        let current_sr = self.output_config.read().as_ref().map(|c| c.sample_rate());
        let new_sr = self
            .output_device()
            .and_then(|d| next_gate.select_output_config(&d))
            .map(|c| c.sample_rate());

        if current_sr != new_sr {
            // Clear cached configs so output_cfg()/input_cfg() re-derive from the new quality gate.
            *self.output_config.write() = None;
            *self.input_config.write() = None;

            let is_output_running = self.output_thread.read().is_some();
            let is_input_running = self.input_thread.read().is_some();

            if is_output_running {
                self.restart_output_stream()?;
            }
            if is_input_running {
                self.restart_input_stream()?;
            }
        }

        Ok(())
    }

    pub fn with_runtime<F: FnOnce(&RuntimeSubsystem) -> common::error::Result<()>>(
        &self,
        f: F,
    ) -> common::error::Result<()> {
        if let Some(rt) = self.runtime.read().as_ref() {
            f(rt)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use crate::create_telemetry_channel;
    use common::playback_quality::PlaybackQuality;

    #[test]
    fn rejects_invalid_host_id_on_input_selection() {
        let (telemetry, _rx) = create_telemetry_channel();
        let engine = Engine::new(telemetry, None, None);

        let result =
            engine.select_input_device("definitely-not-a-host".to_string(), "x".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_host_id_on_output_selection() {
        let (telemetry, _rx) = create_telemetry_channel();
        let engine = Engine::new(telemetry, None, None);

        let result =
            engine.select_output_device("definitely-not-a-host".to_string(), "x".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn quality_change_without_active_streams_is_ok() {
        let (telemetry, _rx) = create_telemetry_channel();
        let engine = Engine::new(telemetry, None, None);

        assert!(engine.on_quality_change(PlaybackQuality::Medium).is_ok());
    }

    #[test]
    fn with_runtime_without_active_runtime_is_ok() {
        let (telemetry, _rx) = create_telemetry_channel();
        let engine = Engine::new(telemetry, None, None);

        let result = engine.with_runtime(|_| Ok(()));
        assert!(result.is_ok());
    }
}
