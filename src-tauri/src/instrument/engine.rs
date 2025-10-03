use common::error::{ControlError, InstrumentError, Result as AppResult};
use common::instrument::Config as InstrumentConfig;
use common::instrument::Layout as InstrumentLayout;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use fundsp::hacker::prelude::*;
use mint::Vector2;
use parking_lot::RwLock;
use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

// Ack timeout for control messages.
const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationSource {
    #[default]
    Entropy,
    Mic,
}

impl From<u8> for ActivationSource {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Entropy,
            _ => Self::Mic,
        }
    }
}

impl From<ActivationSource> for u8 {
    fn from(value: ActivationSource) -> u8 {
        match value {
            ActivationSource::Entropy => 0,
            ActivationSource::Mic => 1,
        }
    }
}

#[derive(Debug)]
enum ControlInvocationResult {
    OkChanged,
    NoOp,
    BackendMissing,
    Error(String),
}

// Control channel messages with per‑invocation response sender.
enum Control {
    Pause(Sender<ControlInvocationResult>),
    Resume(Sender<ControlInvocationResult>),
    Shutdown(Sender<ControlInvocationResult>),
}

#[derive(Default)]
pub struct InstrumentEngine {
    pub(super) inner: Inner,
}

#[derive(Default)]
pub(super) struct Inner {
    playing: RwLock<bool>,
    activation_source: RwLock<ActivationSource>,
    layout: RwLock<InstrumentLayout>,
    config: RwLock<InstrumentConfig>,
    dsp_net_frontend: RwLock<Option<fundsp::hacker32::Net>>,
    // Primary oscillator node id for dynamic replacement.
    dsp_primary_node_id: RwLock<Option<NodeId>>,
    // Device/output sample rate (Hz) captured at stream start.
    sample_rate: RwLock<Option<f64>>,
    control_tx: RwLock<Option<Sender<Control>>>,
    join: RwLock<Option<std::thread::JoinHandle<()>>>,
}

impl Inner {
    pub fn layout(&self) -> ::common::instrument::Layout {
        *self.layout.read()
    }

    /// Build a fresh network based on current config; returns (Net, primary_node_id).
    fn create_network(&self, sample_rate: f64) -> fundsp::hacker32::Net {
        let mut net = fundsp::hacker32::Net::new(0, 2);
        net.set_sample_rate(sample_rate);
        let config_len = self.config.read().0.len();
        let base_freq: f32 = (220.0 + (config_len as f64 * 5.0)) as f32;

        // Left channel (primary tracked node)
        let left_id = net.push(Box::new(sine_hz::<f32>(base_freq)));
        net.pipe_output(left_id);
        // Right channel slight detune
        let right_id = net.push(Box::new(sine_hz::<f32>(base_freq * 1.01)));
        net.pipe_output(right_id);

        net
    }

    /// Replace primary node oscillator based on current config (if playing).
    fn update_primary_node(&self) {
        if !*self.playing.read() {
            return;
        }
        let Some(primary_id) = *self.dsp_primary_node_id.read() else {
            return;
        };
        let mut guard = self.dsp_net_frontend.write();
        let Some(net) = guard.as_mut() else {
            return;
        };

        let sr = self.sample_rate.read().unwrap_or(44100.0);
        let new_node = self.create_network(sr);

        net.crossfade(primary_id, Fade::Smooth, 0.3, Box::new(new_node));
        net.commit();
        log::info!("Updated primary node (sr={sr})");
    }

    pub fn start_playback(&self) -> AppResult<bool> {
        if *self.playing.read() {
            return Ok(false);
        }

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;
        let default_cfg = device
            .default_output_config()
            .map_err(|_| InstrumentError::OutputConfigUnavailable)?;

        let sample_rate = default_cfg.sample_rate().0 as f64;
        *self.sample_rate.write() = Some(sample_rate);
        let subnet = self.create_network(sample_rate);
        let mut net = Net::new(0, 2);
        net.set_sample_rate(sample_rate);

        let main_node_id = net.push(Box::new(subnet));

        net.pipe_output(main_node_id);

        // Split (retain front-end for later mutation).
        let mut backend = net.backend();
        let front = net;

        // Prepare CPAL stream config
        let stream_cfg: cpal::StreamConfig = default_cfg.clone().into();
        let channels = stream_cfg.channels as usize;

        // Control channel root
        let (tx, rx) = mpsc::channel::<Control>();

        // Store front-end & primary node id.
        {
            *self.dsp_net_frontend.write() = Some(front);
            *self.dsp_primary_node_id.write() = Some(main_node_id);
        }

        let next_value = move || backend.get_stereo();

        // Build output stream (per-format handlers; simple, no generic helper).
        let stream_result = Self::run(&device, &stream_cfg, &default_cfg, next_value);

        let stream = stream_result.map_err(|e| InstrumentError::StartFailed {
            detail: Some(e.to_string()),
        })?;

        stream.play().map_err(|e| InstrumentError::StartFailed {
            detail: Some(e.to_string()),
        })?;

        // Spawn thread to own the stream and process control messages.
        let handle = thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                match msg {
                    Control::Pause(ret) => {
                        // TODO: implement real backend pause; currently always changed.
                        let _ = ret.send(ControlInvocationResult::OkChanged);
                    }
                    Control::Resume(ret) => {
                        // TODO: implement real backend resume.
                        let _ = ret.send(ControlInvocationResult::OkChanged);
                    }
                    Control::Shutdown(ret) => {
                        let _ = ret.send(ControlInvocationResult::OkChanged);
                        break;
                    }
                }
            }
            // stream & backend dropped here.
        });

        // Publish control channel + join handle
        {
            *self.control_tx.write() = Some(tx);
            *self.join.write() = Some(handle);
        }

        *self.playing.write() = true;

        Ok(true)
    }

    fn run<F>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        default_cfg: &cpal::SupportedStreamConfig,
        mut next_sample: F,
    ) -> common::error::Result<cpal::Stream>
    where
        F: FnMut() -> (f32, f32) + Send + Sync + 'static,
    {
        let err_cb = |err| log::error!("instrument playback stream error: {err}");

        match default_cfg.sample_format() {
            SampleFormat::F32 => device.build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    Self::write_data(data, 2, &mut next_sample)
                },
                err_cb,
                None,
            ),
            SampleFormat::I16 => device.build_output_stream(
                config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    Self::write_data(data, 2, &mut next_sample)
                },
                err_cb,
                None,
            ),
            SampleFormat::U16 => device.build_output_stream(
                config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    Self::write_data(data, 2, &mut next_sample)
                },
                err_cb,
                None,
            ),
            other => {
                return Err(InstrumentError::UnsupportedSampleFormat(format!(
                    "unsupported_sample_format:{other:?}"
                ))
                .into())
            }
        }
        .map_err(|e| {
            InstrumentError::BuildStream {
                detail: e.to_string(),
            }
            .into()
        })
    }

    fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> (f32, f32))
    where
        T: cpal::SizedSample + cpal::FromSample<f64>,
    {
        for frame in output.chunks_mut(channels) {
            let sample = next_sample();
            let left: T = T::from_sample(sample.0 as f64);
            let right: T = T::from_sample(sample.1 as f64);

            for (channel, sample) in frame.iter_mut().enumerate() {
                if channel & 1 == 0 {
                    *sample = left;
                } else {
                    *sample = right;
                }
            }
        }
    }

    pub fn stop_playback(&self) -> AppResult<bool> {
        if !*self.playing.read() {
            return Ok(false);
        }
        {
            let mut playing = self.playing.write();
            if !*playing {
                return Ok(false);
            }
            *playing = false;
        }

        let tx_opt = self.control_tx.write().take();
        if let Some(tx) = tx_opt {
            let (ack_tx, ack_rx) = mpsc::channel();
            tx.send(Control::Shutdown(ack_tx))
                .map_err(|_| ControlError::ChannelSend {
                    op: "shutdown".into(),
                })?;
            match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(ControlInvocationResult::OkChanged | ControlInvocationResult::NoOp) => {}
                Ok(ControlInvocationResult::Error(e)) => {
                    return Err(InstrumentError::BuildStream { detail: e }.into())
                }
                Ok(ControlInvocationResult::BackendMissing) => {
                    return Err(InstrumentError::BackendMissing {
                        op: "shutdown".into(),
                    }
                    .into())
                }
                Err(_) => {
                    return Err(InstrumentError::AckTimeout {
                        op: "shutdown".into(),
                    }
                    .into())
                }
            }
        } else {
            return Ok(false);
        }

        if let Some(handle) = self.join.write().take() {
            if let Err(_e) = handle.join() {
                return Err(InstrumentError::ThreadJoin {
                    op: "shutdown".into(),
                }
                .into());
            }
        }

        self.dsp_net_frontend.write().take();
        self.dsp_primary_node_id.write().take();

        Ok(true)
    }

    pub fn playing(&self) -> bool {
        *self.playing.read()
    }

    pub fn activation_source(&self) -> u8 {
        (*self.activation_source.read()).into()
    }

    pub fn set_activation_source(&self, requested: ActivationSource) -> AppResult<bool> {
        let mut src = self.activation_source.write();
        if *src != requested {
            *src = requested;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn pause_playback(&self) -> AppResult<bool> {
        if !*self.playing.read() {
            return Ok(false);
        }
        {
            let mut playing = self.playing.write();
            if !*playing {
                return Ok(false);
            }
            *playing = false;
        }

        let Some(tx) = self.control_tx.read().as_ref().cloned() else {
            return Err(ControlError::BackendMissing { op: "pause".into() }.into());
        };
        let (ack_tx, ack_rx) = mpsc::channel();
        tx.send(Control::Pause(ack_tx))
            .map_err(|_| ControlError::ChannelSend { op: "pause".into() })?;
        match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
            Ok(ControlInvocationResult::OkChanged | ControlInvocationResult::NoOp) => Ok(true),
            Ok(ControlInvocationResult::BackendMissing) => {
                Err(ControlError::BackendMissing { op: "pause".into() }.into())
            }
            Ok(ControlInvocationResult::Error(e)) => {
                Err(ControlError::BuildStream { detail: e }.into())
            }
            Err(_) => Err(ControlError::AckTimeout { op: "pause".into() }.into()),
        }
    }

    pub fn resume_playback(&self) -> AppResult<bool> {
        if *self.playing.read() {
            return Ok(false);
        }
        {
            let mut playing = self.playing.write();
            if *playing {
                return Ok(false);
            }
            *playing = true;
        }

        let Some(tx) = self.control_tx.read().as_ref().cloned() else {
            return Err(ControlError::BackendMissing {
                op: "resume".into(),
            }
            .into());
        };
        let (ack_tx, ack_rx) = mpsc::channel();
        tx.send(Control::Resume(ack_tx))
            .map_err(|_| ControlError::ChannelSend {
                op: "resume".into(),
            })?;
        match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
            Ok(ControlInvocationResult::OkChanged | ControlInvocationResult::NoOp) => Ok(true),
            Ok(ControlInvocationResult::BackendMissing) => Err(ControlError::BackendMissing {
                op: "resume".into(),
            }
            .into()),
            Ok(ControlInvocationResult::Error(e)) => {
                Err(ControlError::BuildStream { detail: e }.into())
            }
            Err(_) => Err(ControlError::AckTimeout {
                op: "resume".into(),
            }
            .into()),
        }
    }

    pub fn set_is_dark(&self, is_dark: bool) -> common::error::Result<()> {
        let new_config = {
            let mut layout = self.layout.write();
            layout.scale = if is_dark {
                common::instrument::Scale::In
            } else {
                common::instrument::Scale::Yo
            };
            common::instrument::Config::try_from(*layout)?
        };
        {
            let mut config = self.config.write();
            *config = new_config;
            log::info!(
                "Created new config for [dark: {is_dark}] (len={}): {:#?}",
                config.0.len(),
                *config
            );
        }
        self.update_primary_node();
        Ok(())
    }

    pub fn set_size(&self, width: f64, height: f64) -> common::error::Result<()> {
        let new_config = {
            let mut layout = self.layout.write();
            let scale = layout.scale;
            *layout = common::instrument::Layout {
                scale,
                ..common::instrument::Layout::from_screen_estate(Vector2 {
                    x: width as f32,
                    y: height as f32,
                })
            };
            common::instrument::Config::try_from(*layout)?
        };
        {
            let mut config = self.config.write();
            *config = new_config;
            log::info!(
                "Created new config for [width: {width}, height: {height}] (len={}): {:#?}",
                config.0.len(),
                *config
            );
        }
        self.update_primary_node();
        Ok(())
    }

    pub fn set_safe_area(
        &self,
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
    ) -> common::error::Result<()> {
        let new_config = {
            let mut layout = self.layout.write();
            let space = layout.space;
            let scale = layout.scale;
            *layout = common::instrument::Layout::from_screen_estate_with_safe_area(
                space, top, right, bottom, left,
            );
            layout.scale = scale;
            common::instrument::Config::try_from(*layout)?
        };
        {
            let mut config = self.config.write();
            *config = new_config;
            log::info!(
                "Updated config after safe area change [top: {top}, right: {right}, bottom: {bottom}, left: {left}] (len={}): {:#?}",
                config.0.len(),
                *config
            );
        }
        self.update_primary_node();
        Ok(())
    }
}
