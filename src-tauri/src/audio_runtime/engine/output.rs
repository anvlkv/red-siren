use std::{
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait as _, HostTrait as _},
    Device, StreamConfig, SupportedStreamConfig,
};
use thingbuf::ThingBuf;

use crate::{
    audio_runtime::{
        stream::{
            self, playback_callback, spawn_owned_output_stream, Control, PlaybackCallbackConfig,
            PlaybackQuality, PlaybackQualityGate, ProdData, TelemetrySender,
        },
        AudioRuntimeError,
    },
    dsp::{DspNetwork as _, Synthesizer},
};

const FADE_DURATION_MS: f64 = 50_f64;

pub struct Output {
    pub net: Arc<Synthesizer>,
    pub device: Option<Device>,
    pub config: Option<SupportedStreamConfig>,
    pub thread: Option<thread::JoinHandle<()>>,
    pub control: Option<Sender<Control>>,
    pub current_quality: Arc<parking_lot::RwLock<PlaybackQuality>>,
    pub host: cpal::Host,
    pub telemetry_sender: Option<TelemetrySender>,
    pub input_buffer: Option<Arc<ThingBuf<ProdData>>>,
}

impl Output {
    pub fn new(
        host: cpal::Host,
        current_quality: Arc<parking_lot::RwLock<PlaybackQuality>>,
        telemetry_sender: Option<TelemetrySender>,
        input_buffer: Option<Arc<ThingBuf<ProdData>>>,
        net: Arc<Synthesizer>,
    ) -> Self {
        Self {
            net,
            device: None,
            config: None,
            thread: None,
            control: None,
            current_quality,
            host,
            telemetry_sender,
            input_buffer,
        }
    }

    fn output_device(&self) -> Option<Device> {
        self.device
            .clone()
            .or_else(|| self.host.default_output_device())
    }

    fn cfg(&self) -> Option<SupportedStreamConfig> {
        self.config.or_else(|| {
            self.output_device()
                .and_then(|d| {
                    let q_gate = PlaybackQualityGate::from(*self.current_quality.read());
                    q_gate
                        .select_output_config(&d)
                        .or_else(|| d.default_output_config().ok())
                })
                .or_else(|| self.default_output_cfg())
        })
    }

    fn default_output_cfg(&self) -> Option<SupportedStreamConfig> {
        self.host
            .default_output_device()
            .and_then(|d| d.default_output_config().ok())
    }

    pub fn start(&mut self) -> Result<(), AudioRuntimeError> {
        if let Some((device, default_cfg)) = self.output_device().zip(self.cfg()) {
            let stream_cfg: StreamConfig = default_cfg.into();

            let input_buffer = self.input_buffer.clone();
            let sample_rate = stream_cfg.sample_rate;
            let num_channels = std::cmp::Ord::min(stream_cfg.channels as usize, 2);

            let telemetry = self
                .telemetry_sender
                .clone()
                .ok_or(AudioRuntimeError::NotInitialized)?;

            let backend = self.net.backend();

            let (control, handle) = spawn_owned_output_stream(
                device,
                default_cfg,
                stream_cfg,
                num_channels,
                move || {
                    let cb_cfg = PlaybackCallbackConfig {
                        input_buffer,
                        sample_rate,
                        telemetry,
                        num_channels,
                    };

                    playback_callback(backend, cb_cfg)
                },
            )?;

            self.thread = Some(handle);
            self.control = Some(control);

            self.net.fade_in(FADE_DURATION_MS);

            Ok(())
        } else {
            Err(super::AudioEngineError::NoOutputDeviceAvailable.into())
        }
    }

    pub fn on_quality_change(&mut self) -> bool {
        let new_gate = PlaybackQualityGate::from(*self.current_quality.read());
        if let Some(output_cfg) = self
            .cfg()
            .filter(|&cfg| self.config.is_none_or(|old_cfg| old_cfg != cfg))
        {
            self.net.set_sample_rate(output_cfg.sample_rate());
            self.net.set_sample_type(new_gate.sample_type());
            true
        } else {
            false
        }
    }

    pub fn stop(&mut self) -> Result<(), AudioRuntimeError> {
        self.net.fade_out(FADE_DURATION_MS);

        if let Some((control, handle)) = self.control.take().zip(self.thread.take()) {
            let (ack_tx, ack_rx) = mpsc::channel();

            control
                .send(Control::Shutdown(ack_tx))
                .map_err(|e| stream::AudioStreamError::ControlChannelSend { op: e.to_string() })?;

            match ack_rx.recv_timeout(Duration::from_millis(stream::CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(Ok(_)) => {
                    handle
                        .join()
                        .map_err(|_| stream::AudioStreamError::ThreadJoin {
                            op: "join output thread after successful shutdown".to_string(),
                        })?;
                    Ok(())
                }
                Ok(Err(e)) => {
                    handle
                        .join()
                        .map_err(|_| stream::AudioStreamError::ThreadJoin {
                            op: format!("join output thread after error: {e}"),
                        })?;
                    log::error!("Stopping output stream failed to invoke control: {e}");
                    Ok(())
                }
                Err(e) => {
                    self.thread = Some(handle);
                    self.control = Some(control);

                    Err(stream::AudioStreamError::ControlTimeout {
                        op: format!("stopping output stream: {e}"),
                    }
                    .into())
                }
            }
        } else {
            Err(stream::AudioStreamError::BackendMissing.into())
        }
    }

    pub fn pause(&mut self) -> Result<(), AudioRuntimeError> {
        self.net.fade_out(FADE_DURATION_MS);
        let cx = self
            .control
            .as_ref()
            .ok_or(stream::AudioStreamError::BackendMissing)?;

        let (ack_tx, ack_rx) = mpsc::channel();
        if cx.send(Control::Pause(ack_tx)).is_err() {
            return Err(stream::AudioStreamError::ControlChannelSend {
                op: "pause output".to_string(),
            }
            .into());
        }
        if let Err(e) =
            ack_rx.recv_timeout(Duration::from_millis(stream::CONTROL_INVOKE_TIMEOUT_MS))
        {
            return Err(match e {
                mpsc::RecvTimeoutError::Timeout => stream::AudioStreamError::ControlTimeout {
                    op: "pause output".to_string(),
                },
                mpsc::RecvTimeoutError::Disconnected => {
                    stream::AudioStreamError::ControlChannelSend {
                        op: "pause output (ack channel disconnected)".to_string(),
                    }
                }
            }
            .into());
        }

        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), AudioRuntimeError> {
        self.net.fade_in(FADE_DURATION_MS);
        let cx = self
            .control
            .as_ref()
            .ok_or(stream::AudioStreamError::BackendMissing)?;

        let (ack_tx, ack_rx) = mpsc::channel();
        if cx.send(Control::Resume(ack_tx)).is_err() {
            return Err(stream::AudioStreamError::ControlChannelSend {
                op: "resume output".to_string(),
            }
            .into());
        }
        if let Err(e) =
            ack_rx.recv_timeout(Duration::from_millis(stream::CONTROL_INVOKE_TIMEOUT_MS))
        {
            return Err(match e {
                mpsc::RecvTimeoutError::Timeout => stream::AudioStreamError::ControlTimeout {
                    op: "resume output".to_string(),
                },
                mpsc::RecvTimeoutError::Disconnected => {
                    stream::AudioStreamError::ControlChannelSend {
                        op: "resume output (ack channel disconnected)".to_string(),
                    }
                }
            }
            .into());
        }

        Ok(())
    }
}
