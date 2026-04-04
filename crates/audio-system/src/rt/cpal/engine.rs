use std::{
    f32, mem,
    ops::DerefMut,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

#[cfg(feature = "editor")]
use common::commands::edit::FineTunedValuesPayload;
use common::{
    device::DeviceData,
    error::AppError,
    instrument::{Config as InstrumentConfig, Layout as InstrumentLayout, Preset},
};
use common::{
    error::{ControlError, InstrumentError, Result},
    NodeKey,
};
use common::{instrument::PlaybackQuality, tuner::Config as TunerConfig};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, DeviceId, HostId, StreamConfig, SupportedStreamConfig,
};
use fundsp::prelude::*;
use fundsp::{thingbuf::ThingBuf, typenum::Unsigned};
use mint::Vector2;
use parking_lot::RwLock;
use u_num_it::u_num_it;

use crate::{
    input::analyzer::SpectrumBuffer,
    quality::PlaybackQualityGate,
    rt::{
        cpal::stream::{playback_callback, spawn_owned_input_stream, PlaybackCallbackConfig},
        rt_subsystem::RuntimeSubsystem,
        telemetry::TelemetrySender,
        AudioRuntime, ExcitementSource,
    },
};

use super::stream::{spawn_owned_output_stream, Control};

const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;

const INPUT_BUFFER_DURATION_MS: f64 = 20_f64;
const SPECTRUM_BUFFER_CAPACITY: usize = 2;

/// CPAL-backed stream controller implementing audio I/O and DSP graph
/// lifecycle. The higher-level runtime facade instantiates this when the
/// `rt_cpal` feature is enabled.
struct CpalController {
    runtime: RwLock<RuntimeSubsystem>,
    spectrum_buffer: RwLock<SpectrumBuffer>,
    telemetry_buffer: RwLock<TelemetrySender>,
    quality_gate: Arc<RwLock<PlaybackQualityGate>>,

    output_device: RwLock<Option<Device>>,
    output_config: RwLock<Option<SupportedStreamConfig>>,
    output_thread: RwLock<Option<thread::JoinHandle<()>>>,
    output_control: RwLock<Option<Sender<Control>>>,
    input_device: RwLock<Option<Device>>,
    input_config: RwLock<Option<SupportedStreamConfig>>,
    input_thread: RwLock<Option<thread::JoinHandle<()>>>,
    input_control: RwLock<Option<Sender<Control>>>,
    input_buffer: RwLock<Option<Arc<ThingBuf<f32>>>>,
}

impl CpalController {
    pub fn new(
        telemetry: TelemetrySender,
        preset: Option<Preset>,
        output_device: Option<DeviceData>,
        input_device: Option<DeviceData>,
    ) -> Self {
        let preset = preset.unwrap_or_default();
        let input_device = input_device
            .and_then(Self::device_from_data)
            .or_else(Self::default_input_device);
        let output_device = output_device
            .and_then(Self::device_from_data)
            .or_else(Self::default_output_device);

        let spectrum_buffer = SpectrumBuffer::new(ThingBuf::new(SPECTRUM_BUFFER_CAPACITY));
        let layout = InstrumentLayout::from_screen_estate(Vector2 {
            x: 1280.0,
            y: 720.0,
        });
        let config = InstrumentConfig::try_from(layout).unwrap_or_else(|err| {
            log::error!("Failed to derive instrument config from default layout: {err}");
            InstrumentConfig::default()
        });

        let output_config = output_device
            .as_ref()
            .and_then(|d| d.default_output_config().ok());
        let quality_gate = output_config
            .as_ref()
            .cloned()
            .map(PlaybackQualityGate::from)
            .unwrap_or_default();

        let runtime = {
            let sample_type = quality_gate.sample_type();
            let sample_rate = quality_gate.sample_rate(None);

            RuntimeSubsystem::new(
                layout,
                config,
                ExcitementSource::default(),
                TunerConfig::default(),
                preset,
                spectrum_buffer.clone(),
                sample_type,
                2,
                sample_rate as f64,
            )
        };

        Self {
            runtime: RwLock::new(runtime),
            spectrum_buffer: RwLock::new(spectrum_buffer),
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
        }
    }

    fn device_from_data(data: DeviceData) -> Option<Device> {
        let host_id: HostId = data.host_id.parse().ok()?;
        let device_id: DeviceId = DeviceId(host_id, data.device_id);

        cpal::host_from_id(host_id)
            .ok()
            .and_then(|host| host.device_by_id(&device_id))
    }

    pub fn list_devices(&self) -> Vec<DeviceData> {
        cpal::available_hosts()
            .into_iter()
            .filter_map(|host_id| {
                let host = cpal::host_from_id(host_id).ok()?;
                host.devices().ok()
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
        let host_id: HostId = host_id
            .parse()
            .map_err(|_| InstrumentError::HostUnavailable)?;
        let device_id: DeviceId = DeviceId(host_id, device_id);
        let host = cpal::host_from_id(host_id).map_err(|_| InstrumentError::HostUnavailable)?;
        let device = host
            .device_by_id(&device_id)
            .ok_or(InstrumentError::InvalidDeviceId)?;
        if !device.supports_input() {
            return Err(AppError::Instrument(InstrumentError::NoSupportForInput));
        }

        let _prev_device = self.input_device.write().replace(device);

        self.restart_input_stream()?;

        Ok(())
    }

    pub fn select_output_device(&self, host_id: String, device_id: String) -> Result<()> {
        let host_id: HostId = host_id
            .parse()
            .map_err(|_| InstrumentError::HostUnavailable)?;
        let device_id: DeviceId = DeviceId(host_id, device_id);
        let host = cpal::host_from_id(host_id).map_err(|_| InstrumentError::HostUnavailable)?;
        let device = host
            .device_by_id(&device_id)
            .ok_or(InstrumentError::InvalidDeviceId)?;
        if !device.supports_output() {
            return Err(AppError::Instrument(InstrumentError::NoSupportForOutput));
        }
        let _prev_device = self.output_device.write().replace(device);

        self.restart_output_stream()?;

        Ok(())
    }

    fn input_device(&self) -> Option<Device> {
        self.input_device
            .read()
            .clone()
            .or_else(Self::default_input_device)
    }

    fn default_input_device() -> Option<Device> {
        cpal::default_host().default_input_device()
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
                .or_else(Self::default_input_cfg)
        })
    }

    fn default_input_cfg() -> Option<SupportedStreamConfig> {
        Self::default_input_device().and_then(|d| d.default_input_config().ok())
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
            Err(AppError::Instrument(InstrumentError::NoSupportForInput))
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

            control.send(Control::Shutdown(ack_tx)).map_err(|e| {
                InstrumentError::Control(ControlError::ChannelSend { op: e.to_string() })
            })?;

            match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(Ok(_)) => {
                    handle.join().map_err(|_| {
                        InstrumentError::Control(ControlError::ThreadJoin {
                            op: "join input thread aftre successful shutdown".to_string(),
                        })
                    })?;
                    Ok(())
                }
                Ok(Err(e)) => {
                    handle.join().map_err(|_| {
                        InstrumentError::Control(ControlError::ThreadJoin {
                            op: format!("join input thread after error: {e}"),
                        })
                    })?;
                    log::error!("Stopping input stream failed to invoke control: {e}");
                    Ok(())
                }
                Err(e) => {
                    *self.input_thread.write() = Some(handle);
                    *self.input_control.write() = Some(control);
                    *self.input_buffer.write() = Some(bf);

                    Err(AppError::Instrument(InstrumentError::AckTimeout {
                        op: format!("stopping input stream: {e}"),
                    }))
                }
            }
        } else {
            Err(AppError::Instrument(InstrumentError::BackendMissing {
                op: "stop input stream".to_string(),
            }))
        }
    }

    fn restart_input_stream(&self) -> Result<()> {
        self.stop_input_stream()?;
        self.start_input_stream()?;
        Ok(())
    }

    fn output_device(&self) -> Option<Device> {
        self.output_device
            .read()
            .clone()
            .or_else(Self::default_output_device)
    }

    fn default_output_device() -> Option<Device> {
        cpal::default_host().default_output_device()
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
                .or_else(Self::default_output_cfg)
        })
    }

    fn default_output_cfg() -> Option<SupportedStreamConfig> {
        Self::default_output_device().and_then(|d| d.default_output_config().ok())
    }

    fn start_output_stream(
        &self,
        layout: InstrumentLayout,
        config: InstrumentConfig,
        source: ExcitementSource,
        tuner_config: TunerConfig,
        preset: Preset,
    ) -> Result<()> {
        if self.output_thread.read().is_some() {
            return Ok(());
        }

        if let Some((device, default_cfg)) = self.output_device().zip(self.output_cfg()) {
            let stream_cfg: StreamConfig = default_cfg.clone().into();

            let input_buffer = self.input_buffer.read().clone();
            let sample_rate = stream_cfg.sample_rate;
            let num_channels = stream_cfg.channels as usize;
            let (buffer_target_frames, sample_type, quality) = {
                let qg = self.quality_gate.read();
                (
                    qg.buffer_size(None) as usize,
                    qg.sample_type(),
                    self.quality_gate.clone(),
                )
            };
            let spectrum_data_thb = self.spectrum_buffer.read().clone();
            let telemetry = self.telemetry_buffer.read().clone();
            let backend = {
                let sys = RuntimeSubsystem::new(
                    layout,
                    config,
                    source,
                    tuner_config,
                    preset,
                    spectrum_data_thb,
                    sample_type,
                    num_channels,
                    stream_cfg.sample_rate as f64,
                );
                let backend = sys.backend();
                *self.runtime.write() = sys;
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
                        1..=7,
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

            self.runtime.read().fade_in();

            Ok(())
        } else {
            Err(AppError::Instrument(InstrumentError::NoSupportForOutput))
        }
    }

    fn stop_output_stream(&self) -> Result<()> {
        let rt = self.runtime.read();

        rt.fade_out();

        if let Some((control, handle)) = self
            .output_control
            .write()
            .take()
            .zip(self.output_thread.write().take())
        {
            let (ack_tx, ack_rx) = mpsc::channel();

            control.send(Control::Shutdown(ack_tx)).map_err(|e| {
                InstrumentError::Control(ControlError::ChannelSend { op: e.to_string() })
            })?;

            match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(Ok(_)) => {
                    handle.join().map_err(|_| {
                        InstrumentError::Control(ControlError::ThreadJoin {
                            op: "join output thread aftre successful shutdown".to_string(),
                        })
                    })?;
                    Ok(())
                }
                Ok(Err(e)) => {
                    handle.join().map_err(|_| {
                        InstrumentError::Control(ControlError::ThreadJoin {
                            op: format!("join output thread after error: {e}"),
                        })
                    })?;
                    log::error!("Stopping output stream failed to invoke control: {e}");
                    Ok(())
                }
                Err(e) => {
                    *self.output_thread.write() = Some(handle);
                    *self.output_control.write() = Some(control);

                    Err(AppError::Instrument(InstrumentError::AckTimeout {
                        op: format!("stopping output stream: {e}"),
                    }))
                }
            }
        } else {
            Err(AppError::Instrument(InstrumentError::BackendMissing {
                op: "stop output stream".to_string(),
            }))
        }
    }

    fn restart_output_stream(&self) -> Result<()> {
        let rt = self.runtime.write();

        rt.fade_out();
        let layout = *rt.layout.read();
        let config = rt.config.read().clone();
        let source = *rt.source.read();
        let tuner_config = rt.tuner_config.read().clone();
        let preset = rt.preset.read().clone();

        drop(rt);

        self.stop_output_stream()?;
        self.start_output_stream(layout, config, source, tuner_config, preset)?;
        Ok(())
    }
}

impl AudioRuntime for CpalController {
    fn start(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        source: ExcitementSource,
        tuner_config: &TunerConfig,
        preset: Preset,
    ) -> common::error::Result<()> {
        if matches!(source, ExcitementSource::Mic) {
            self.start_input_stream()?;
        }

        self.start_output_stream(
            *layout,
            config.clone(),
            source,
            tuner_config.clone(),
            preset,
        )?;

        Ok(())
    }

    fn stop(&self) -> common::error::Result<()> {
        self.stop_input_stream().or_else(|e| {
            if matches!(
                e,
                AppError::Instrument(InstrumentError::BackendMissing { .. })
            ) {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        self.stop_output_stream()?;
        Ok(())
    }

    fn pause(&self) -> common::error::Result<()> {
        {
            let runtime = self.runtime.read();
            runtime.fade_out();
        }

        {
            let output_control = self.output_control.read();
            let Some(cx) = output_control.as_ref() else {
                return Err(AppError::Instrument(InstrumentError::NotInitialized));
            };
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Pause(ack_tx)).is_err() {
                return Err(AppError::Instrument(InstrumentError::Control(
                    ControlError::ChannelSend {
                        op: "Pause output".to_string(),
                    },
                )));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::Instrument(InstrumentError::Control(match e {
                    mpsc::RecvTimeoutError::Timeout => ControlError::AckTimeout {
                        op: "Pause output".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => ControlError::ChannelSend {
                        op: "Pause output".to_string(),
                    },
                })));
            }
        }

        if let Some(cx) = self.input_control.read().as_ref() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Pause(ack_tx)).is_err() {
                return Err(AppError::Instrument(InstrumentError::Control(
                    ControlError::ChannelSend {
                        op: "Pause input".to_string(),
                    },
                )));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::Instrument(InstrumentError::Control(match e {
                    mpsc::RecvTimeoutError::Timeout => ControlError::AckTimeout {
                        op: "Pause input".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => ControlError::ChannelSend {
                        op: "Pause input".to_string(),
                    },
                })));
            }
        }

        Ok(())
    }

    fn resume(&self) -> common::error::Result<()> {
        if let Some(cx) = self.input_control.read().as_ref() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Resume(ack_tx)).is_err() {
                return Err(AppError::Instrument(InstrumentError::Control(
                    ControlError::ChannelSend {
                        op: "Resume input".to_string(),
                    },
                )));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::Instrument(InstrumentError::Control(match e {
                    mpsc::RecvTimeoutError::Timeout => ControlError::AckTimeout {
                        op: "Resume input".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => ControlError::ChannelSend {
                        op: "Resume input".to_string(),
                    },
                })));
            }
        }

        {
            let output_control = self.output_control.read();
            let Some(cx) = output_control.as_ref() else {
                return Err(AppError::Instrument(InstrumentError::NotInitialized));
            };
            let (ack_tx, ack_rx) = mpsc::channel();
            if cx.send(Control::Resume(ack_tx)).is_err() {
                return Err(AppError::Instrument(InstrumentError::Control(
                    ControlError::ChannelSend {
                        op: "Resume output".to_string(),
                    },
                )));
            }
            if let Err(e) = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                return Err(AppError::Instrument(InstrumentError::Control(match e {
                    mpsc::RecvTimeoutError::Timeout => ControlError::AckTimeout {
                        op: "Resume output".to_string(),
                    },
                    mpsc::RecvTimeoutError::Disconnected => ControlError::ChannelSend {
                        op: "Resume output".to_string(),
                    },
                })));
            }
        }

        {
            let runtime = self.runtime.read();
            runtime.fade_in();
        }

        Ok(())
    }

    fn on_excitement_source_changed(&self, source: ExcitementSource) -> common::error::Result<()> {
        let old_source = {
            let rt = self.runtime.read();
            let mut src_x = rt.source.write();
            let mut src_y = source;
            mem::swap(src_x.deref_mut(), &mut src_y);
            src_y
        };

        match (old_source, source) {
            (ExcitementSource::Mic, ExcitementSource::Entropy) => {
                self.stop_input_stream()?;
                self.restart_output_stream()?;
                Ok(())
            }
            (ExcitementSource::Entropy, ExcitementSource::Mic) => {
                self.start_input_stream()?;
                self.restart_output_stream()?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn on_layout_changed(
        &self,
        instrument_layout: &InstrumentLayout,
        instrument_config: &InstrumentConfig,
        tuner_config: &TunerConfig,
    ) -> common::error::Result<()> {
        let rt = self.runtime.read();
        rt.update_configurations(instrument_config, instrument_layout, tuner_config);
        Ok(())
    }

    fn update_quality_setting(&self, qg: PlaybackQualityGate) {
        *self.quality_gate.write() = qg;
    }

    fn snapshot_output_snoop(&self, key: NodeKey) -> Vec<f32> {
        self.runtime.read().snapshot_output_snoop(key)
    }

    fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
        self.runtime.read().snapshot_all_output_snoops()
    }

    fn snapshot_excitement_snoop(&self, key: NodeKey) -> Vec<(f32, f32)> {
        self.runtime.read().snapshot_excitement_snoop(key)
    }

    fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)> {
        self.runtime.read().snapshot_all_excitement_snoops()
    }

    fn snapshot_processed_output_spectrum(
        &self,
    ) -> common::error::Result<Option<crate::rt::ProcessedOutputSpectrumSnapshot>> {
        self.runtime.read().snapshot_processed_output_spectrum()
    }

    fn snapshot_input_snoop(&self) -> Vec<f32> {
        self.runtime.read().snapshot_input_snoop()
    }

    fn set_band_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()> {
        let runtime = self.runtime.read();
        runtime.set_band_control(key, value)
    }

    fn get_band_control(&self, key: common::NodeKey) -> common::error::Result<f32> {
        let runtime = self.runtime.read();
        runtime.get_band_control(key)
    }

    fn set_key_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()> {
        let runtime = self.runtime.read();
        runtime.set_key_control(key, value)
    }

    fn get_key_control(&self, key: common::NodeKey) -> common::error::Result<f32> {
        let runtime = self.runtime.read();
        runtime.get_key_control(key)
    }

    fn start_tuner_only(&self, tuner_config: &TunerConfig) -> common::error::Result<()> {
        if self.output_thread.read().is_some() {
            self.stop_output_stream()?;
        }

        let mut rt = self.runtime.write();
        *rt = rt.restart_with_tuner_only(Some(tuner_config.clone()));

        if self.input_thread.read().is_none() {
            self.start_input_stream()?;
        }

        Ok(())
    }

    fn poll_tuner_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot> {
        self.spectrum_buffer.read().pop().map(|spectrum| {
            common::tuner::SpectrumSnapshot(
                spectrum
                    .data()
                    .iter()
                    .map(|(freq, mag)| (freq.val(), mag.val()))
                    .collect(),
            )
        })
    }

    fn start_tap_tuner_audio(&self) -> common::error::Result<()> {
        self.runtime.read().start_tap_tuner_audio();
        Ok(())
    }

    fn stop_tap_tuner_audio(&self) -> common::error::Result<()> {
        self.runtime.read().stop_tap_tuner_audio();
        Ok(())
    }

    fn update_tuner_config(&self, tuner_config: &TunerConfig) -> common::error::Result<()> {
        let runtime = self.runtime.read();
        runtime.update_tuner_config(tuner_config);
        Ok(())
    }

    fn poll_tuner_excitements(&self) -> Vec<(NodeKey, f32)> {
        self.runtime.read().poll_tuner_excitements()
    }

    fn get_sample_rate(&self) -> f64 {
        if let Some(cfg) = self.output_config.read().clone() {
            cfg.sample_rate() as f64
        } else {
            self.quality_gate.read().sample_rate(None) as f64
        }
    }

    fn quality_indicator(&self) -> PlaybackQuality {
        (*self.quality_gate.read()).into()
    }

    fn set_preset(&self, preset: Preset) -> common::error::Result<()> {
        let runtime = self.runtime.read();
        runtime.update_preset(preset);
        Ok(())
    }

    fn get_preset(&self) -> Preset {
        let runtime = self.runtime.read();
        runtime.get_preset()
    }

    #[cfg(feature = "editor")]
    fn get_finetuned_values(&self) -> common::error::Result<FineTunedValuesPayload> {
        let runtime = self.runtime.read();
        runtime.get_finetuned_values()
    }

    #[cfg(feature = "editor")]
    fn set_finetuned_values(&self, payload: FineTunedValuesPayload) -> common::error::Result<()> {
        let runtime = self.runtime.read();
        runtime.set_finetuned_values(payload)
    }
}

// impl Default for CpalController {
//     fn default() -> Self {
//         let default_tuner_cfg = TunerConfig::default();
//         Self {
//             dsp_net_frontend: RwLock::new(None),
//             dsp_primary_node_id: RwLock::new(None),
//             sample_rate: RwLock::new(None),
//             gain_param: RwLock::new(None),
//             processed_output_snoops: RwLock::new(None),
//             tuner_tap_gain_param: RwLock::new(None),
//             freq_range: RwLock::new((
//                 common::instrument::consts::SOFT_MIN_FREQ_HZ,
//                 common::instrument::consts::SOFT_MAX_FREQ_HZ,
//             )),
//             #[cfg(feature = "editor")]
//             fine_tuned_shared_values: RwLock::new(FineTunedSharedValues::default()),
//             input_snoop: RwLock::new(None),
//             tuner_freq_range: Arc::new((shared(f32::NEG_INFINITY), shared(f32::INFINITY))),
//             tuner_ny_threshold: Arc::new(shared(default_tuner_cfg.ny_threshold)),
//             tuner_ny_wet_ratio: Arc::new(shared(default_tuner_cfg.ny_wet_ratio)),
//             control_tx: RwLock::new(None),
//             output_thread: RwLock::new(None),
//             input_sender: RwLock::new(None),
//             input_thread: RwLock::new(None),
//             node_excitement_snoops: RwLock::new(HashMap::new()),
//             node_output_snoops: RwLock::new(HashMap::new()),
//             preset: RwLock::new(Preset::default()),
//             node_band_controls: RwLock::new(HashMap::new()),
//             node_key_controls: RwLock::new(HashMap::new()),
//             node_sensor_controls: RwLock::new(HashMap::new()),
//             spectrum_data_thb: Arc::new(ThingBuf::new(SPECTRUM_BUFFER_CAPACITY)),
//             siren_excitements: RwLock::new(HashMap::new()),
//             output_device: RwLock::new(None),
//             input_device: RwLock::new(None),
//             tuner_only_mode: Arc::new(RwLock::new(false)),

//             quality_gate: RwLock::new(PlaybackQualityGate::default()),
//         }
//     }
// }

// impl CpalController {
//     fn evaluate_gate_and_maybe_restart(&self) -> bool {
//         // Auto quality must be enabled
//         if !*self.auto_quality.read() {
//             return false;
//         }

//         // Access telemetry from the output stream
//         let telemetry_arc_opt = self.output_telemetry.read().clone();
//         let telemetry_arc = match telemetry_arc_opt {
//             Some(t) => t,
//             None => return false,
//         };
//         let (ema_slack, recent_underruns) = if let Some(t) = telemetry_arc.try_lock() {
//             (t.ema_compute_slack_ns, t.local_underruns > 0)
//         } else {
//             // Skip evaluation this tick if telemetry is busy to avoid blocking RT callback
//             return false;
//         };

//         // Evaluate recommendation
//         let now = std::time::Instant::now();
//         let current_gate = *self.quality_gate.read();
//         let mut gm = self.gate_manager.write();

//         if let Some(new_gate) =
//             gm.evaluate_recommendation(current_gate, ema_slack, recent_underruns, now)
//         {
//             // Fade out and shutdown current streams before restart.
//             self.fade_out();
//             if let Err(e) = self.shutdown_streams() {
//                 log::error!("shutdown_streams failed during gate restart: {e}");
//                 return false;
//             }

//             // Update gate and indicator to the new value.
//             {
//                 let mut g = self.quality_gate.write();
//                 *g = new_gate;
//             }
//             self.quality_indicator
//                 .store(new_gate as i8, std::sync::atomic::Ordering::Relaxed);

//             // Delegate full rebuild to existing start() path using stored last_* state.
//             if let Err(e) = self.restart_with_current_state_and_gate() {
//                 log::error!("restart_with_current_state_and_gate failed: {e}");
//                 return false;
//             }

//             // Reset GateManager timers after successful restart.
//             gm.reset_timers(now);

//             return true;
//         }

//         false
//     }

//     fn output_device(&self) -> Option<cpal::Device> {
//         { self.output_device.read().clone() }.or_else(|| {
//             let host = cpal::default_host();
//             log::trace!("using CPAL host: {}", host.id().name());
//             let device = host.default_output_device();

//             {
//                 *self.output_device.write() = device.clone();
//             }

//             device
//         })
//     }

//     fn sample_rate(&self) -> f64 {
//         { *self.sample_rate.read() }
//             .or_else(|| {
//                 self.output_device()
//                     .and_then(|d| d.default_output_config().ok())
//                     .map(|c| {
//                         let sample_rate = c.sample_rate();
//                         {
//                             *self.sample_rate.write() = Some(sample_rate as f64);
//                         }
//                         sample_rate as f64
//                     })
//             })
//             .unwrap_or(44100.0)
//     }

//     fn input_device(&self) -> Option<cpal::Device> {
//         { self.input_device.read().clone() }
//             .or_else(|| {
//                 let device = self.output_device().filter(|d| d.supports_input());

//                 {
//                     *self.input_device.write() = device.clone();
//                 }

//                 device
//             })
//             .or_else(|| {
//                 let host = cpal::default_host();
//                 log::trace!("using CPAL host: {}", host.id().name());
//                 let device = host.default_input_device();

//                 {
//                     *self.input_device.write() = device.clone();
//                 }

//                 device
//             })
//     }

//     fn start_input_stream(&self) -> Result<Arc<ThingBuf<f32>>> {
//         let input_device = self
//             .input_device()
//             .ok_or(InstrumentError::DeviceUnavailable)?;

//         let input_device_name = input_device.id().unwrap();
//         log::debug!("input device: {}", input_device_name);
//         // Feature-driven selection of input config; fallback to device default.
//         let input_default_cfg = self
//             .quality_gate
//             .read()
//             .select_input_config(&input_device)
//             .ok_or(InstrumentError::InputConfigUnavailable)?;

//         // Get sample rate from InputStreamManager or use output rate
//         let input_sr = input_default_cfg.sample_rate() as f64;

//         let samples_per_ms = input_sr / 1000.0;
//         let cap_samples = (samples_per_ms * INPUT_BUFFER_DURATION_MS as f64).ceil() as usize;
//         let capacity = cap_samples.next_power_of_two();

//         let thb = Arc::new(ThingBuf::<f32>::new(capacity));

//         let stream_cfg = input_default_cfg.config();

//         // stream_cfg.buffer_size

//         let prod = thb.clone();

//         let (input_sx, input_handle) =
//             spawn_owned_input_stream(input_device, input_default_cfg, stream_cfg, move || {
//                 Box::new(move |sample: &f32| {
//                     if prod.push(*sample).is_err() {
//                         _ = prod.pop();
//                         _ = prod.push(*sample);
//                     }
//                 }) as Box<ProdType>
//             })?;

//         // Persist input thread / control handles.
//         {
//             *self.input_sender.write() = Some(input_sx);
//             *self.input_thread.write() = Some(input_handle);
//         }

//         Ok(thb)
//     }

//     fn start_output_stream(
//         &self,
//         input_buffer: Option<Arc<ThingBuf<f32>>>,
//         backend: NetBackend,
//     ) -> Result<()> {
//         let output_device = self
//             .output_device()
//             .ok_or(InstrumentError::DeviceUnavailable)?;

//         let output_default_cfg = self
//             .quality_gate
//             .read()
//             .select_output_config(&output_device)
//             .ok_or(InstrumentError::OutputConfigUnavailable)?;

//         let stream_cfg: cpal::StreamConfig = output_default_cfg.clone().into();

//         let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;

//         let quality: std::sync::Arc<std::sync::atomic::AtomicI8> = self.quality_indicator.clone();
//         let sr = {
//             *self.sample_rate.write() = Some(stream_cfg.sample_rate as f64);
//             stream_cfg.sample_rate
//         };
//         let no_reset = self.tuner_only_mode.clone();
//         // Create shared telemetry handle and seed sample rate.
//         let telemetry = std::sync::Arc::new(parking_lot::Mutex::new(
//             crate::rt::cpal::stream::telemetry::PlaybackTelemetry {
//                 sample_rate: sr,
//                 ..Default::default()
//             },
//         ));
//         {
//             *self.output_telemetry.write() = Some(telemetry.clone());
//         }
//         // Spawn output stream owner.
//         let (tx, handle) = spawn_owned_output_stream(
//             output_device,
//             output_default_cfg,
//             stream_cfg,
//             output_channels,
//             {
//                 let telemetry = telemetry.clone();
//                 let buffer_target_frames = self.quality_gate.read().buffer_size(None);
//                 move || {
//                     let cfg = super::stream::playback::PlaybackCallbackConfig {
//                         input_buffer: input_buffer.clone(),
//                         quality: quality.clone(),
//                         no_reset_on_silence: no_reset.clone(),
//                         sample_rate: sr,
//                         buffer_target_frames: buffer_target_frames as usize,
//                         telemetry: telemetry.clone(),
//                     };
//                     playback_callback(backend, cfg)
//                 }
//             },
//         )?;

//         // Persist output thread / control handles.
//         {
//             *self.control_tx.write() = Some(tx);
//             *self.output_thread.write() = Some(handle);
//         }

//         // Gate management evaluation is invoked from AudioRuntime::start via a periodic loop.

//         Ok(())
//     }

//     fn shutdown_streams(&self) -> Result<()> {
//         // Stop gate evaluation worker and clear reset channel
//         self.gate_eval_running
//             .store(false, std::sync::atomic::Ordering::SeqCst);
//         if let Some(handle) = self.gate_eval_thread.write().take() {
//             let _ = handle.join();
//         }
//         _ = self.gate_reset_tx.write().take();

//         // Take thread handles
//         _ = self.output_thread.write().take();
//         _ = self.input_thread.write().take();

//         // Take DSP
//         _ = self.dsp_net_frontend.write().take();
//         _ = self.dsp_primary_node_id.write().take();
//         _ = self.gain_param.write().take();
//         _ = self.sample_rate.write().take();

//         // Clear snoops & controls
//         self.node_excitement_snoops.write().clear();
//         self.node_output_snoops.write().clear();
//         self.node_band_controls.write().clear();
//         self.node_key_controls.write().clear();
//         self.node_sensor_controls.write().clear();

//         // Take control channels
//         let control_tx = self.control_tx.write().take();
//         let input_sender = self.input_sender.write().take();

//         // Output stream shutdown
//         if let Some(tx) = control_tx {
//             let (ack_tx, ack_rx) = mpsc::channel();
//             tx.send(Control::Shutdown(ack_tx))
//                 .map_err(|_| ControlError::ChannelSend {
//                     op: "shutdown".into(),
//                 })?;
//             match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
//                 Ok(ControlInvocationResult::Ok(_)) => {}
//                 Ok(ControlInvocationResult::Err(e)) => {
//                     return Err(InstrumentError::BuildStream { detail: e }.into())
//                 }
//                 Err(_) => {
//                     return Err(InstrumentError::AckTimeout {
//                         op: "shutdown".into(),
//                     }
//                     .into())
//                 }
//             }
//         }

//         // Excitement stream shutdown
//         if let Some(act_tx) = input_sender {
//             let (ack_tx, ack_rx) = mpsc::channel();
//             if act_tx.send(Control::Shutdown(ack_tx)).is_ok() {
//                 let _ = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS));
//             }
//         }

//         Ok(())
//     }

//     /// Set band control value for a specific node
//     fn set_band_control(&self, key: NodeKey, value: f32) -> Result<()> {
//         self.preset.write().set_band_value(&key, value);
//         let band_controls = self.node_band_controls.read();
//         if let Some(control) = band_controls.get(&key) {
//             control.set_value(value);
//             Ok(())
//         } else {
//             Err(ControlError::NodeNotFound { key }.into())
//         }
//     }

//     /// Get band control value for a specific node
//     fn get_band_control(&self, key: NodeKey) -> Result<f32> {
//         let band_controls = self.node_band_controls.read();
//         if let Some(control) = band_controls.get(&key) {
//             Ok(control.value())
//         } else {
//             Err(ControlError::NodeNotFound { key }.into())
//         }
//     }

//     /// Set key control value for a specific node (0.0 = false/released, 1.0 = true/pressed)
//     fn set_key_control(&self, key: NodeKey, value: f32) -> Result<()> {
//         self.preset.write().set_key_value(&key, value);
//         let key_controls = self.node_key_controls.read();
//         if let Some(control) = key_controls.get(&key) {
//             control.set_value(value);
//             Ok(())
//         } else {
//             Err(ControlError::NodeNotFound { key }.into())
//         }
//     }

//     /// Get key control value for a specific node (0.0 = false/released, 1.0 = true/pressed)
//     fn get_key_control(&self, key: NodeKey) -> Result<f32> {
//         let key_controls = self.node_key_controls.read();
//         if let Some(control) = key_controls.get(&key) {
//             Ok(control.value())
//         } else {
//             Err(ControlError::NodeNotFound { key }.into())
//         }
//     }

//     fn set_tuner_input_values(&self, tuner_config: &TunerConfig) {
//         self.tuner_freq_range
//             .0
//             .set_value(tuner_config.frequency_range.0.unwrap_or(f32::NEG_INFINITY));
//         self.tuner_freq_range
//             .1
//             .set_value(tuner_config.frequency_range.1.unwrap_or(f32::INFINITY));
//         self.tuner_ny_threshold.set_value(tuner_config.ny_threshold);
//         self.tuner_ny_wet_ratio.set_value(tuner_config.ny_wet_ratio);
//     }
// }

// impl CpalController {
//     fn restart_with_current_state_and_gate(&self) -> Result<()> {
//         // Smoothly transition: fade out, shutdown, then restart using last known state.
//         self.fade_out();
//         self.shutdown_streams()?;

//         // Recreate streams using the stored last_* state.
//         let layout = *self.last_layout.read();
//         let config = self.last_config.read().clone();
//         let source = *self.last_source.read();
//         let tuner_config = self.last_tuner_config.read().clone();

//         // Delegate to existing start flow which handles device/config selection and wiring.
//         self.start(&layout, &config, source, &tuner_config)?;

//         // Fade back in after successful restart.
//         self.fade_in();

//         Ok(())
//     }
// }

// impl AudioRuntime for CpalController {
//     fn start(
//         &self,
//         layout: &InstrumentLayout,
//         config: &InstrumentConfig,
//         source: ExcitementSource,
//         tuner_config: &TunerConfig,
//     ) -> Result<()> {
//         log::trace!("CpalController.start: begin with source={:?}", source);

//         self.set_tuner_input_values(tuner_config);

//         // Ensure audio session is configured (iOS)
//         audio_session::ensure_configured()?;

//         // Snapshot for potential restarts.
//         {
//             *self.last_layout.write() = *layout;
//             *self.last_config.write() = config.clone();
//             *self.last_source.write() = source;
//             *self.last_tuner_config.write() = tuner_config.clone();
//             log::trace!("sotred layout/config/source/tuner_config");
//         }

//         let output_device = self
//             .output_device()
//             .ok_or(InstrumentError::DeviceUnavailable)?;
//         let output_device_name = output_device.id().unwrap();
//         log::trace!(
//             "CpalController.start: default output device: {}",
//             output_device_name
//         );
//         let output_default_cfg = output_device
//             .default_output_config()
//             .map_err(|_| InstrumentError::OutputConfigUnavailable)?;
//         log::trace!(
//             "CpalController.start: output default cfg: format={:?}, channels={}, sample_rate={} Hz, buffer={:?}",
//             output_default_cfg.sample_format(),
//             output_default_cfg.channels(),
//             output_default_cfg.sample_rate(),
//             output_default_cfg.buffer_size(),
//         );

//         // Currently support up to stereo.
//         let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;
//         log::trace!(
//             "CpalController.start: using {} output channels",
//             output_channels
//         );

//         // Prepare excitement ring buffer & excitement source thread.
//         log::trace!(
//             "CpalController.start: selecting excitement source branch: {:?}",
//             source
//         );

//         let input_buffer = if matches!(source, ExcitementSource::Mic) {
//             log::debug!("CpalController.start: using Mic excitement");
//             Some(self.start_input_stream()?)
//         } else {
//             None
//         };

//         let subnet = self.create_instrument_network(config, tuner_config, source);

//         let mut net = self.create_main_network(output_channels, subnet);

//         // Split -> backend tick side & retained frontend mutation side.
//         let backend = net.backend();
//         let front = net;

//         self.start_output_stream(input_buffer, backend)?;

//         // Store references
//         {
//             *self.dsp_net_frontend.write() = Some(front);
//         }

//         // Spawn gate evaluation worker thread (non-blocking; uses only Arcs)
//         {
//             let telemetry_opt = self.output_telemetry.read().clone();
//             if let Some(telemetry) = telemetry_opt {
//                 let auto_quality = self.auto_quality.clone();
//                 let quality_indicator = self.quality_indicator.clone();
//                 let start_gate = *self.quality_gate.read();
//                 thread::spawn(move || {
//                     let mut gm = super::gate_manager::GateManager::new(start_gate);
//                     let mut current_gate = start_gate;
//                     loop {
//                         thread::sleep(Duration::from_millis(300));
//                         if !*auto_quality.read() {
//                             continue;
//                         }
//                         if let Some(t) = telemetry.try_lock() {
//                             let ema_slack = t.ema_compute_slack_ns;
//                             let recent_underruns = t.local_underruns > 0;
//                             drop(t);
//                             let now = std::time::Instant::now();
//                             if let Some(new_gate) = gm.evaluate_recommendation(
//                                 current_gate,
//                                 ema_slack,
//                                 recent_underruns,
//                                 now,
//                             ) {
//                                 current_gate = new_gate;
//                                 quality_indicator
//                                     .store(new_gate as i8, std::sync::atomic::Ordering::Relaxed);
//                                 // TODO: send reset signal via a channel (gate reset tx) owned by engine
//                             }
//                         }
//                     }
//                 });
//             }
//         }

//         Ok(())
//     }

//     fn stop(&self) -> Result<()> {
//         log::trace!("CpalController.stop: begin");
//         self.fade_out();

//         let res = self.shutdown_streams();
//         if res.is_ok() {
//             log::trace!("CpalController.stop: shutdown_streams Ok");
//         } else {
//             log::error!("CpalController.stop: shutdown_streams Err");
//         }
//         res
//     }

//     fn pause(&self) -> Result<()> {
//         log::trace!("CpalController.pause: begin");
//         self.fade_out();
//         let Some(tx) = self.control_tx.read().as_ref().cloned() else {
//             return Err(ControlError::BackendMissing { op: "pause".into() }.into());
//         };
//         let (ack_tx, ack_rx) = mpsc::channel();
//         tx.send(Control::Pause(ack_tx))
//             .map_err(|_| ControlError::ChannelSend { op: "pause".into() })?;
//         match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
//             Ok(ControlInvocationResult::Ok(_)) => Ok(()),
//             Ok(ControlInvocationResult::Err(e)) => {
//                 Err(ControlError::BuildStream { detail: e }.into())
//             }
//             Err(_) => Err(ControlError::AckTimeout { op: "pause".into() }.into()),
//         }
//     }

//     fn resume(&self) -> Result<()> {
//         log::trace!("CpalController.resume: begin");
//         let Some(tx) = self.control_tx.read().as_ref().cloned() else {
//             return Err(ControlError::BackendMissing {
//                 op: "resume".into(),
//             }
//             .into());
//         };
//         let (ack_tx, ack_rx) = mpsc::channel();
//         tx.send(Control::Resume(ack_tx))
//             .map_err(|_| ControlError::ChannelSend {
//                 op: "resume".into(),
//             })?;
//         match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
//             Ok(ControlInvocationResult::Ok(_)) => {
//                 self.fade_in();
//                 Ok(())
//             }
//             Ok(ControlInvocationResult::Err(e)) => {
//                 Err(ControlError::BuildStream { detail: e }.into())
//             }
//             Err(_) => Err(ControlError::AckTimeout {
//                 op: "resume".into(),
//             }
//             .into()),
//         }
//     }

//     fn on_excitement_source_changed(&self, source: ExcitementSource) -> Result<()> {
//         log::trace!(
//             "CpalController.on_excitement_source_changed: requested={:?}",
//             source
//         );
//         *self.last_source.write() = source;
//         let started = self.control_tx.read().is_some();
//         log::trace!(
//             "CpalController.on_excitement_source_changed: controller started? {}",
//             started
//         );
//         if !started {
//             log::trace!("CpalController.on_excitement_source_changed: backend not started; caching source and returning Ok");
//             return Ok(());
//         }
//         // Restart streaming pipeline with new source
//         log::trace!("CpalController.on_excitement_source_changed: preparing to restart streams");
//         let layout = *self.last_layout.read();
//         let config = self.last_config.read().clone();
//         let tuner_config = self.last_tuner_config.read().clone();
//         log::trace!("CpalController.on_excitement_source_changed: state cloned; calling stop()");
//         self.stop()?;
//         log::trace!(
//             "CpalController.on_excitement_source_changed: stop() returned Ok; calling start()"
//         );
//         self.start(&layout, &config, source, &tuner_config)
//     }

//     fn on_layout_changed(
//         &self,
//         layout: &InstrumentLayout,
//         config: &InstrumentConfig,
//         tuner_config: &TunerConfig,
//     ) -> Result<()> {
//         log::info!(
//             "Layout changed: groups={}, keys_per_group={}",
//             layout.num_groups.get(),
//             layout.num_keys_per_group.get()
//         );

//         self.set_tuner_input_values(tuner_config);

//         // Validate NodeKey consistency between configs
//         let registry = layout.registry();

//         // Check for invalid NodeKeys in tuner config
//         let sensor_keys: std::collections::HashMap<NodeKey, ()> = tuner_config
//             .sensor_data
//             .iter()
//             .map(|s| (s.key, ()))
//             .collect();
//         let invalid_sensor_keys = registry.has_invalid_keys(&sensor_keys);
//         if !invalid_sensor_keys.is_empty() {
//             log::warn!("Invalid sensor NodeKeys found: {:?}", invalid_sensor_keys);
//         }

//         *self.last_layout.write() = *layout;
//         *self.last_config.write() = config.clone();
//         *self.last_tuner_config.write() = tuner_config.clone();
//         if self.control_tx.read().is_some() {
//             self.update_primary_node(config, tuner_config);
//         } else {
//             log::warn!("Audio stream not running, layout change will apply on next start");
//         }
//         Ok(())
//     }

//     fn snapshot_output_snoop(&self, node_key: NodeKey) -> Vec<f32> {
//         let mut out = Vec::new();
//         let mut snoops = self.node_output_snoops.write();
//         if let Some(snoop) = snoops.get_mut(&node_key) {
//             snoop.update();
//             let cap = snoop.capacity();
//             out.reserve(cap + 2);
//             for rev in (0..cap).rev() {
//                 let s = snoop.at(rev);
//                 if s.is_normal() || s == 0.0 {
//                     out.push(snoop.at(rev));
//                 } else {
//                     out.push(0.0);
//                 }
//             }
//         }
//         out
//     }

//     fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
//         let layout = *self.last_layout.read();
//         let registry = layout.registry();

//         registry
//             .all_keys()
//             .iter()
//             .map(|&node_key| (node_key, self.snapshot_output_snoop(node_key)))
//             .collect()
//     }

//     fn snapshot_excitement_snoop(&self, node_key: NodeKey) -> Vec<(f32, f32)> {
//         let mut out = Vec::new();
//         let mut snoops = self.node_excitement_snoops.write();
//         if let Some((primary, secondary)) = snoops.get_mut(&node_key) {
//             primary.update();
//             secondary.update();
//             let cap = primary.capacity();
//             out.reserve(cap + 2);
//             for rev in (0..cap).rev() {
//                 let p = primary.at(rev);
//                 let s = secondary.at(rev);
//                 // avoid denormals in output
//                 if (p.is_normal() || p == 0.0) && (s.is_normal() || s == 0.0) {
//                     out.push((primary.at(rev), secondary.at(rev)));
//                 } else {
//                     out.push((0.0, 0.0));
//                 }
//             }
//         }
//         out
//     }

//     fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)> {
//         let layout = *self.last_layout.read();
//         let registry = layout.registry();

//         let mut result = Vec::with_capacity(registry.total_keys());

//         registry.iter_keys(|node_key| {
//             result.push((node_key, self.snapshot_excitement_snoop(node_key)))
//         });
//         result
//     }

//     fn snapshot_processed_output_spectrum(
//         &self,
//     ) -> Result<Option<crate::rt::ProcessedOutputSpectrumSnapshot>> {
//         // Snapshot sample rate and frequency range using short-lived locks
//         let sample_rate = match self.sample_rate.read().as_ref() {
//             Some(sr) => *sr,
//             None => return Ok(None),
//         };
//         let (min_hz, max_hz) = *self.freq_range.read();

//         // Local windows to fill without holding locks during heavy processing
//         let mut l_window = [0.0; OUTPUT_ANALYZER_FFT_WINDOW_SIZE];
//         let mut r_window = [0.0; OUTPUT_ANALYZER_FFT_WINDOW_SIZE];

//         // Pull available buffers quickly; avoid sleeping while holding locks
//         let filled = {
//             if let Some((l, r)) = self.processed_output_snoops.write().as_mut() {
//                 let mut len = 0;
//                 while len < OUTPUT_ANALYZER_FFT_WINDOW_SIZE {
//                     if let Some((l_buffer, r_buffer)) = l.get().zip(r.get()) {
//                         let remaining = OUTPUT_ANALYZER_FFT_WINDOW_SIZE - len;
//                         let num_samples = std::cmp::Ord::min(l_buffer.size(), remaining);
//                         for i in 0..num_samples {
//                             l_window[len + i] = l_buffer.at(i);
//                             r_window[len + i] = r_buffer.at(i);
//                         }
//                         len += num_samples;
//                     } else {
//                         // Not enough data available right now; bail out early
//                         break;
//                     }
//                 }
//                 len
//             } else {
//                 0
//             }
//         };

//         // If buffers were not ready, let caller try again later
//         if filled < OUTPUT_ANALYZER_FFT_WINDOW_SIZE {
//             return Ok(None);
//         }

//         // Perform analysis without holding any locks
//         let left_spectrum =
//             output_analyzer::analyze(l_window, sample_rate, min_hz as f32, max_hz as f32)?;
//         let right_spectrum =
//             output_analyzer::analyze(r_window, sample_rate, min_hz as f32, max_hz as f32)?;

//         Ok(Some((left_spectrum, right_spectrum)))
//     }

//     fn set_band_control(&self, key: NodeKey, value: f32) -> Result<()> {
//         self.set_band_control(key, value)
//     }

//     fn get_band_control(&self, key: NodeKey) -> Result<f32> {
//         self.get_band_control(key)
//     }

//     fn set_key_control(&self, key: NodeKey, value: f32) -> Result<()> {
//         self.set_key_control(key, value)
//     }

//     fn get_key_control(&self, key: NodeKey) -> Result<f32> {
//         self.get_key_control(key)
//     }

//     fn poll_tuner_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot> {
//         self.spectrum_data_thb.pop().map(|d| {
//             common::tuner::SpectrumSnapshot(
//                 d.data()
//                     .iter()
//                     .map(|(freq, mag)| (freq.val(), mag.val()))
//                     .collect(),
//             )
//         })
//     }

//     fn poll_tuner_excitements(&self) -> Vec<(NodeKey, f32)> {
//         let mut data = self
//             .siren_excitements
//             .read()
//             .iter()
//             .map(|(k, v)| (*k, v.value::<f32>().re))
//             .collect::<Vec<_>>();
//         data.sort_by_key(|(k, _)| *k);

//         data
//     }

//     fn start_tuner_only(&self, tuner_config: &TunerConfig) -> common::error::Result<()> {
//         self.set_tuner_input_values(tuner_config);

//         {
//             *self.last_tuner_config.write() = tuner_config.clone();
//         }

//         let input_stream = self.start_input_stream()?;

//         let subnet = self.create_tuner_only_network(tuner_config);

//         let output_device = self
//             .output_device()
//             .ok_or(InstrumentError::DeviceUnavailable)?;

//         let output_default_cfg = output_device
//             .default_output_config()
//             .map_err(|_| InstrumentError::OutputConfigUnavailable)?;

//         let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;
//         log::trace!(
//             "CpalController.start: using {} output channels",
//             output_channels
//         );

//         let mut net = self.create_main_network(output_channels, subnet);

//         let backend = net.backend();
//         let front = net;

//         self.start_output_stream(Some(input_stream), backend)?;

//         {
//             *self.dsp_net_frontend.write() = Some(front);
//         }

//         Ok(())
//     }

//     fn start_tap_tuner_audio(&self) -> common::error::Result<()> {
//         if let Some(tuner_tap_gain_param) = self.tuner_tap_gain_param.read().as_ref() {
//             tuner_tap_gain_param.set_value(1.0);
//             Ok(())
//         } else {
//             Err(TunerError::MissingParameter("tap gain".to_string()).into())
//         }
//     }

//     fn stop_tap_tuner_audio(&self) -> common::error::Result<()> {
//         if let Some(tuner_tap_gain_param) = self.tuner_tap_gain_param.read().as_ref() {
//             tuner_tap_gain_param.set_value(0.0);
//             Ok(())
//         } else {
//             Err(TunerError::MissingParameter("tap gain".to_string()).into())
//         }
//     }

//     fn update_tuner_config(&self, new_config: &TunerConfig) -> common::error::Result<()> {
//         let old_config = self.last_tuner_config.read().clone();
//         {
//             *self.last_tuner_config.write() = new_config.clone();
//         }
//         if old_config
//             .sensor_data
//             .iter()
//             .map(|d| d.key)
//             .collect::<Vec<_>>()
//             != new_config
//                 .sensor_data
//                 .iter()
//                 .map(|d| d.key)
//                 .collect::<Vec<_>>()
//         {
//             self.update_primary_node(&self.last_config.read(), new_config);
//         } else {
//             let controls = self.node_sensor_controls.read();
//             for sensor_data in &new_config.sensor_data {
//                 if let Some(ctrl) = controls.get(&sensor_data.key) {
//                     ctrl.max_frequency.set_value(sensor_data.max_frequency);
//                     ctrl.min_frequency.set_value(sensor_data.min_frequency);
//                     ctrl.max_magnitude.set_value(sensor_data.max_magnitude);
//                     ctrl.min_magnitude.set_value(sensor_data.min_magnitude);
//                     log::info!("Updated tuner sensor control for key {:?}", sensor_data.key);
//                 } else {
//                     return Err(TunerError::MissingParameter(format!(
//                         "controls for node key: {:?}",
//                         sensor_data.key
//                     ))
//                     .into());
//                 }
//             }
//         }

//         self.set_tuner_input_values(new_config);

//         Ok(())
//     }

//     fn get_sample_rate(&self) -> f64 {
//         self.sample_rate()
//     }

//     fn quality_indicator(&self) -> PlaybackQuality {
//         (*self.quality_gate.read()).into()
//     }

//     fn set_preset(&self, preset: Preset) -> common::error::Result<()> {
//         self.node_band_controls.write().retain(|k, _| preset.has(k));
//         self.node_key_controls.write().retain(|k, _| preset.has(k));

//         self.node_band_controls
//             .read()
//             .iter()
//             .for_each(|(key, shared)| {
//                 if let Some(val) = preset.get_band_value(key) {
//                     shared.set_value(val);
//                 }
//             });
//         self.node_key_controls
//             .read()
//             .iter()
//             .for_each(|(key, shared)| {
//                 if let Some(val) = preset.get_key_value(key) {
//                     shared.set_value(val);
//                 }
//             });
//         for &key in preset
//             .keys()
//             .filter(|&k| !self.node_band_controls.read().contains_key(k))
//         {
//             self.node_band_controls
//                 .write()
//                 .insert(key, shared(preset.get_band_value(&key).unwrap()));
//         }
//         for &key in preset
//             .keys()
//             .filter(|&k| !self.node_key_controls.read().contains_key(k))
//         {
//             self.node_key_controls
//                 .write()
//                 .insert(key, shared(preset.get_key_value(&key).unwrap()));
//         }
//         *self.preset.write() = preset;

//         Ok(())
//     }

//     fn get_preset(&self) -> Preset {
//         self.preset.read().clone()
//     }

//     fn snapshot_input_snoop(&self) -> Vec<f32> {
//         self.input_snoop
//             .write()
//             .as_mut()
//             .map(|snoop| {
//                 snoop.update();
//                 let cap = snoop.capacity();
//                 let mut out = Vec::with_capacity(cap + 2);
//                 for rev in (0..cap).rev() {
//                     let s = snoop.at(rev);
//                     if s.is_normal() || s == 0.0 {
//                         out.push(snoop.at(rev));
//                     } else {
//                         out.push(0.0);
//                     }
//                 }
//                 out
//             })
//             .unwrap_or_default()
//     }

//     #[cfg(feature = "editor")]
//     fn get_finetuned_values(&self) -> Result<common::commands::edit::FineTunedValuesPayload> {
//         let shared_values = self.fine_tuned_shared_values.read();
//         Ok(common::commands::edit::FineTunedValuesPayload {
//             siren_alpha: shared_values.siren_alpha.value(),
//             group_q: shared_values.group_q.value(),
//             group_ls_gain_db: shared_values.group_ls_gain_db.value(),
//             filter_morph_follow_s: shared_values.filter_morph_follow_s.value(),
//             node_follow_response_time_s: shared_values.node_follow_response_time_s.value(),
//             filter_q_piercing: shared_values.filter_q_piercing.value(),
//             filter_q_bright: shared_values.filter_q_bright.value(),
//             filter_q_shelf: shared_values.filter_q_shelf.value(),
//             filter_shelf_gain_db: shared_values.filter_shelf_gain_db.value(),
//             filter_q_warm: shared_values.filter_q_warm.value(),
//             node_bell_q: shared_values.node_bell_q.value(),
//             node_bell_gain_db: shared_values.node_bell_gain_db.value(),
//             formant_base_q: shared_values.formant_base_q.value(),
//         })
//     }

//     #[cfg(feature = "editor")]
//     #[allow(clippy::too_many_arguments)]
//     fn set_finetuned_values(
//         &self,
//         payload: common::commands::edit::FineTunedValuesPayload,
//     ) -> Result<()> {
//         {
//             let shared_values = self.fine_tuned_shared_values.write();
//             shared_values.siren_alpha.set_value(payload.siren_alpha);
//             shared_values
//                 .node_follow_response_time_s
//                 .set_value(payload.node_follow_response_time_s);
//             shared_values.group_q.set_value(payload.group_q);
//             shared_values
//                 .group_ls_gain_db
//                 .set_value(payload.group_ls_gain_db);
//             shared_values
//                 .filter_morph_follow_s
//                 .set_value(payload.filter_morph_follow_s);
//             shared_values
//                 .filter_q_piercing
//                 .set_value(payload.filter_q_piercing);
//             shared_values
//                 .filter_q_bright
//                 .set_value(payload.filter_q_bright);
//             shared_values
//                 .filter_q_shelf
//                 .set_value(payload.filter_q_shelf);
//             shared_values
//                 .filter_shelf_gain_db
//                 .set_value(payload.filter_shelf_gain_db);
//             shared_values.filter_q_warm.set_value(payload.filter_q_warm);
//             shared_values.node_bell_q.set_value(payload.node_bell_q);
//             shared_values
//                 .node_bell_gain_db
//                 .set_value(payload.node_bell_gain_db);
//             shared_values
//                 .formant_base_q
//                 .set_value(payload.formant_base_q);
//         }

//         self.update_primary_node(&self.last_config.read(), &self.last_tuner_config.read());

//         Ok(())
//     }
// }
/// Factory exposed to the runtime facade.
pub fn make_stream_controller(
    telemetry: TelemetrySender,
    preset: Option<Preset>,
    output_device: Option<DeviceData>,
    input_device: Option<DeviceData>,
) -> Result<Box<dyn AudioRuntime + Send + Sync>> {
    Ok(Box::new(CpalController::new(
        telemetry,
        preset,
        output_device,
        input_device,
    )))
}
