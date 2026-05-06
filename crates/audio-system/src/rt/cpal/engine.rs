use std::{
    f32,
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
    quality::{PlaybackQualityGate, SampleType},
    rt::{
        cpal::stream::{playback_callback, spawn_owned_input_stream, PlaybackCallbackConfig},
        rt_subsystem::RuntimeSubsystem,
        telemetry::TelemetrySender,
        AudioRuntime, ExcitementSource,
    },
    system::excitor::SpectrumBuffer,
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
        let spectrum_buffer = runtime.tuner_spectrum_buffer();

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
                let spectrum_buffer = sys.tuner_spectrum_buffer();

                let backend = sys.backend();
                *self.runtime.write() = sys;
                *self.spectrum_buffer.write() = spectrum_buffer;
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
        let old_source = *self.runtime.read().source.read();
        if old_source == source {
            return Ok(());
        }

        // Manage the microphone input stream first so that input_buffer is
        // already populated (or absent) by the time the output stream restarts.
        match (old_source, source) {
            (ExcitementSource::Mic, ExcitementSource::Entropy) => {
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
            }
            (ExcitementSource::Entropy, ExcitementSource::Mic) => {
                self.start_input_stream()?;
            }
            _ => {}
        }

        if self.output_thread.read().is_some() {
            // A full DSP rebuild is required here. fundsp's Net::backend() can
            // only be called once per Net instance – it moves the vertex list
            // into the backend and the frontend cannot produce a second backend.
            // restart_output_stream_keep_runtime (which called backend() again)
            // would therefore panic. We also need the new input_buffer value
            // (set above) captured fresh in the callback closure, which a full
            // restart guarantees.
            let rt = self.runtime.read();
            let layout = *rt.layout.read();
            let config = rt.config.read().clone();
            let tuner_config = rt.tuner_config.read().clone();
            let preset = rt.preset.read().clone();
            drop(rt);
            self.stop_output_stream()?;
            self.start_output_stream(layout, config, source, tuner_config, preset)?;
        } else {
            // Stream not running: persist the new source so the next
            // start_output_stream picks it up.
            *self.runtime.read().source.write() = source;
        }

        Ok(())
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
        Vec::new()
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

    fn hit_test_node(
        &self,
        key: common::NodeKey,
        frequency: f32,
        excite_real: f32,
        excite_imag: f32,
    ) -> common::error::Result<()> {
        self.runtime
            .read()
            .hit_test_node(key, frequency, excite_real, excite_imag)
    }

    fn release_test_node(&self, key: common::NodeKey) -> common::error::Result<()> {
        self.runtime.read().release_test_node(key)
    }

    fn start_tuner_only(&self, tuner_config: &TunerConfig) -> common::error::Result<()> {
        if self.output_thread.read().is_some() {
            self.stop_output_stream()?;
        }

        let mut rt = self.runtime.write();
        *rt = rt.restart_with_tuner_only(Some(tuner_config.clone()));
        *self.spectrum_buffer.write() = rt.tuner_spectrum_buffer();

        if self.input_thread.read().is_none() {
            self.start_input_stream()?;
        }

        Ok(())
    }

    fn poll_tuner_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot> {
        self.runtime.read().poll_tuner_spectrum()
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

    fn is_running(&self) -> bool {
        self.output_thread.read().is_some()
    }

    fn current_sample_type(&self) -> SampleType {
        self.quality_gate.read().sample_type()
    }

    fn restart_for_quality(&self) -> common::error::Result<()> {
        if self.output_thread.read().is_some() {
            self.restart_output_stream()?;
        }
        Ok(())
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
