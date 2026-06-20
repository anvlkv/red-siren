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
    Device, Host, StreamConfig, SupportedStreamConfig,
};
use thingbuf::ThingBuf;

use crate::{
    audio_runtime::{
        stream::{
            self, spawn_owned_input_stream, Control, PlaybackQuality, PlaybackQualityGate, ProdData,
        },
        AudioRuntimeError,
    },
    dsp::{Analyzer, DspNetwork as _},
};

const INPUT_BUFFER_DURATION_MS: f64 = 15_f64;

pub struct Input {
    pub net: Arc<Analyzer>,
    pub device: Option<Device>,
    pub config: Option<SupportedStreamConfig>,
    pub thread: Option<thread::JoinHandle<()>>,
    pub control: Option<Sender<Control>>,
    pub buffer: Option<Arc<ThingBuf<ProdData>>>,
    pub current_quality: Arc<parking_lot::RwLock<PlaybackQuality>>,
    pub host: Host,
}

impl Input {
    pub fn new(
        host: Host,
        current_quality: Arc<parking_lot::RwLock<PlaybackQuality>>,
        net: Arc<Analyzer>,
    ) -> Self {
        Self {
            net,
            device: None,
            config: None,
            thread: None,
            control: None,
            buffer: None,
            current_quality,
            host,
        }
    }

    fn device(&self) -> Option<Device> {
        self.device
            .clone()
            .or_else(|| self.host.default_input_device())
    }

    fn cfg(&self) -> Option<SupportedStreamConfig> {
        self.config.clone().or_else(|| {
            self.device()
                .and_then(|d| {
                    let q_gate = PlaybackQualityGate::from(*self.current_quality.read());
                    q_gate
                        .select_input_config(&d)
                        .or_else(|| d.default_input_config().ok())
                })
                .or_else(|| self.default_cfg())
        })
    }

    fn default_cfg(&self) -> Option<SupportedStreamConfig> {
        self.host
            .default_input_device()
            .and_then(|d| d.default_input_config().ok())
    }

    pub fn start(&mut self) -> Result<(), AudioRuntimeError> {
        if let Some((device, default_cfg)) = self.device().zip(self.cfg()) {
            let mut backend = self.net.backend();

            let stream_cfg: StreamConfig = default_cfg.into();
            let input_buffer = Arc::new(ThingBuf::<ProdData>::new(
                ((stream_cfg.sample_rate as f64 / 1000_f64) * INPUT_BUFFER_DURATION_MS).round()
                    as usize,
            ));
            self.buffer = Some(input_buffer.clone());

            let (control, handle) =
                spawn_owned_input_stream(device.clone(), default_cfg, stream_cfg, move || {
                    Box::new(move |&(sample, ts)| {
                        let mut processed = [sample];
                        backend.process(&[sample], &mut processed);
                        if let Err(e) = input_buffer.push((processed[0], ts)) {
                            _ = input_buffer.pop();
                            input_buffer.push(e.into_inner()).unwrap();
                        }
                    })
                })?;

            self.thread = Some(handle);
            self.control = Some(control);

            self.device = Some(device.clone());
            self.config = Some(default_cfg);
            Ok(())
        } else {
            Err(super::AudioEngineError::NoInputDeviceAvailable.into())
        }
    }

    pub fn on_quality_change(&mut self) {
        let new_gate = PlaybackQualityGate::from(*self.current_quality.read());
        if let Some(input_cfg) = self.cfg() {
            self.net.set_sample_rate(input_cfg.sample_rate());
            self.net.set_sample_type(new_gate.sample_type());
        }
    }

    pub fn stop(&mut self) -> Result<(), AudioRuntimeError> {
        if let Some(((bf, control), handle)) = self
            .buffer
            .take()
            .zip(self.control.take())
            .zip(self.thread.take())
        {
            let (ack_tx, ack_rx) = mpsc::channel();

            control
                .send(Control::Shutdown(ack_tx))
                .map_err(|e| stream::AudioStreamError::ControlChannelSend { op: e.to_string() })?;

            match ack_rx.recv_timeout(Duration::from_millis(stream::CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(Ok(_)) => {
                    handle
                        .join()
                        .map_err(|_| stream::AudioStreamError::ThreadJoin {
                            op: "join input thread after successful shutdown".to_string(),
                        })?;
                    Ok(())
                }
                Ok(Err(e)) => {
                    handle
                        .join()
                        .map_err(|_| stream::AudioStreamError::ThreadJoin {
                            op: format!("join input thread after error: {e}"),
                        })?;
                    log::error!("Stopping input stream failed to invoke control: {e}");
                    Ok(())
                }
                Err(e) => {
                    self.thread = Some(handle);
                    self.control = Some(control);
                    self.buffer = Some(bf);

                    Err((stream::AudioStreamError::ControlTimeout {
                        op: format!("stopping input stream: {e}"),
                    })
                    .into())
                }
            }
        } else {
            Err(stream::AudioStreamError::BackendMissing.into())
        }
    }

    pub fn pause(&mut self) -> Result<(), AudioRuntimeError> {
        let cx = self
            .control
            .as_ref()
            .ok_or(stream::AudioStreamError::BackendMissing)?;
        let (ack_tx, ack_rx) = mpsc::channel();
        if cx.send(Control::Pause(ack_tx)).is_err() {
            return Err(stream::AudioStreamError::ControlChannelSend {
                op: "pause input".to_string(),
            }
            .into());
        }
        if let Err(e) =
            ack_rx.recv_timeout(Duration::from_millis(stream::CONTROL_INVOKE_TIMEOUT_MS))
        {
            return Err(match e {
                mpsc::RecvTimeoutError::Timeout => stream::AudioStreamError::ControlTimeout {
                    op: "pause input".to_string(),
                },
                mpsc::RecvTimeoutError::Disconnected => {
                    stream::AudioStreamError::ControlChannelSend {
                        op: "pause input (ack channel disconnected)".to_string(),
                    }
                }
            }
            .into());
        }

        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), AudioRuntimeError> {
        let cx = self
            .control
            .as_ref()
            .ok_or(stream::AudioStreamError::BackendMissing)?;
        let (ack_tx, ack_rx) = mpsc::channel();
        if cx.send(Control::Resume(ack_tx)).is_err() {
            return Err(stream::AudioStreamError::ControlChannelSend {
                op: "resume input".to_string(),
            }
            .into());
        }
        if let Err(e) =
            ack_rx.recv_timeout(Duration::from_millis(stream::CONTROL_INVOKE_TIMEOUT_MS))
        {
            return Err(match e {
                mpsc::RecvTimeoutError::Timeout => stream::AudioStreamError::ControlTimeout {
                    op: "resume input".to_string(),
                },
                mpsc::RecvTimeoutError::Disconnected => {
                    stream::AudioStreamError::ControlChannelSend {
                        op: "resume input (ack channel disconnected)".to_string(),
                    }
                }
            }
            .into());
        }

        Ok(())
    }
}
