use std::{
    collections::HashMap,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use common::instrument::Config as InstrumentConfig;
use common::instrument::Layout as InstrumentLayout;
use common::{
    error::{ControlError, InstrumentError, Result as AppResult},
    NodeKey,
};
use cpal::traits::{DeviceTrait, HostTrait};
use fundsp::hacker32::prelude::*;
use mint::Vector2;
use parking_lot::RwLock;
use ringbuf::{
    storage::Heap,
    traits::{Consumer, Producer, Split},
    SharedRb,
};

use super::stream::{
    spawn_owned_input_stream, spawn_owned_noise_stream, spawn_owned_output_stream, Control,
    ControlInvocationResult, GenType, ProdType,
};

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

const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;
const FADE_DURATION_MS: u64 = 120;
const FOLLOW_RESPONSE_SECS: f32 = 0.01;
const BUFFER_DURATION_MS: u64 = 100;

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
    tuner_data: RwLock<common::tuner::Config>,
    dsp_net_frontend: RwLock<Option<Net>>,
    // Primary oscillator node id for dynamic replacement.
    dsp_primary_node_id: RwLock<Option<NodeId>>,
    // Device/output sample rate (Hz) captured at stream start.
    sample_rate: RwLock<Option<f64>>,
    gain_param: RwLock<Option<Shared>>,
    control_tx: RwLock<Option<Sender<Control>>>,
    join: RwLock<Option<thread::JoinHandle<()>>>,
    // Deconstructed NodeHandles from output system
    activation_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    output_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    siren_controls: RwLock<HashMap<NodeKey, Shared>>,
    band_controls: RwLock<HashMap<NodeKey, Shared>>,
    // Activation system handles
    activation_sender: RwLock<Option<Sender<Control>>>,
    activation_thread: RwLock<Option<thread::JoinHandle<()>>>,
}

impl Inner {
    pub fn layout(&self) -> ::common::instrument::Layout {
        *self.layout.read()
    }

    /// Build a fresh network based on current config; returns (Net, primary_node_id).
    fn create_network(&self, sample_rate: f64) -> Net {
        let mut net = Net::new(1, 2);
        net.set_sample_rate(sample_rate);
        // let config_len = self.config.read().0.len();
        // let base_freq: f32 = (220.0 + (config_len as f64 * 5.0)) as f32;

        // Create output system and get handles
        let node_handles = super::system::create_output_system(&self.config.read(), &mut net, 2);

        // Deconstruct and store NodeHandles
        {
            let mut activation_snoops = self.activation_snoops.write();
            let mut output_snoops = self.output_snoops.write();
            let mut siren_controls = self.siren_controls.write();
            let mut band_controls = self.band_controls.write();

            activation_snoops.clear();
            output_snoops.clear();
            siren_controls.clear();
            band_controls.clear();

            // Create HashMap for siren controls to pass to input system

            for handle in node_handles {
                activation_snoops.insert(handle.key, handle.activation_snoop);
                output_snoops.insert(handle.key, handle.output_snoop);
                siren_controls.insert(handle.key, handle.siren_control);
                band_controls.insert(handle.key, handle.band_control);
            }

            // TODO: Create input system when tuner data is ready
            // For now, skip the input system creation as tuner data is being reworked
            {
                let tuner_data = self.tuner_data.read();

                super::system::create_input_system(&tuner_data, &mut net, &siren_controls);
            }
        }

        net.allocate();

        log::debug!("created network: {}", net.display());

        net
    }

    /// Replace primary node oscillator based on current config (if playing).
    fn update_primary_node(&self) {
        // Extract values with narrow lock scopes
        if !*self.playing.read() {
            return;
        }

        let primary_id = match *self.dsp_primary_node_id.read() {
            Some(id) => id,
            None => {
                log::warn!("No primary node id to update");
                return;
            }
        };

        let sample_rate = self.sample_rate.read().unwrap_or(44100.0);

        // Create new node before acquiring write lock
        let new_node = self.create_network(sample_rate);

        // Now acquire write lock for minimal time
        let mut guard = self.dsp_net_frontend.write();
        let Some(net) = guard.as_mut() else {
            log::warn!("No DSP network frontend to update");
            return;
        };

        net.crossfade(primary_id, Fade::Smooth, 0.3, Box::new(new_node));
        net.check();
        net.commit();
    }

    pub fn start_playback(&self) -> AppResult<bool> {
        if *self.playing.read() {
            return Ok(false);
        }

        let host = cpal::default_host();
        let output_device = host
            .default_output_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;
        let output_default_cfg = output_device
            .default_output_config()
            .map_err(|_| InstrumentError::OutputConfigUnavailable)?;

        // FIXME: handle more than 2 channels
        let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;

        let activation_src = ActivationSource::from(self.activation_source());

        let (act_sx, act_handle, activator_cons) =
            if matches!(activation_src, ActivationSource::Mic) {
                let input_device = host
                    .default_input_device()
                    .ok_or(InstrumentError::DeviceUnavailable)?;
                let input_default_cfg = input_device
                    .default_input_config()
                    .map_err(|_| InstrumentError::InputConfigUnavailable)?;

                // Prepare CPAL stream config
                let stream_cfg: cpal::StreamConfig = input_default_cfg.clone().into();

                let channels_in = stream_cfg.channels as usize;
                let samples_per_ms = stream_cfg.sample_rate.0 as f64 / 1000.0;
                let cap_samples = ((samples_per_ms * BUFFER_DURATION_MS as f64).ceil() as usize)
                    .saturating_mul(channels_in);
                let capacity = cap_samples.next_power_of_two();

                let (mut buffer_prod, buffer_cons) = SharedRb::<Heap<f64>>::new(capacity).split();

                let (sx, join) = spawn_owned_input_stream(
                    input_device,
                    input_default_cfg,
                    stream_cfg,
                    move || {
                        // Downmix to mono before pushing into the ring buffer.
                        let ch = channels_in;
                        let mut mono = Vec::<f64>::new();

                        let prod = move |sample: &[f64]| -> usize {
                            if ch <= 1 {
                                buffer_prod.push_slice(sample)
                            } else {
                                mono.clear();
                                mono.reserve(sample.len() / ch);
                                for frame in sample.chunks(ch) {
                                    let sum: f64 = frame.iter().copied().sum();
                                    mono.push(sum / ch as f64);
                                }
                                buffer_prod.push_slice(&mono)
                            }
                        };

                        let boxed: Box<ProdType> = Box::new(prod);

                        boxed
                    },
                )?;

                (sx, join, buffer_cons)
            } else {
                let channels_in = 1usize;
                let samples_per_ms = output_default_cfg.sample_rate().0 as f64 / 1000.0;
                let cap_samples = ((samples_per_ms * BUFFER_DURATION_MS as f64).ceil() as usize)
                    .saturating_mul(channels_in);
                let capacity = cap_samples.next_power_of_two();

                let (mut buffer_prod, buffer_cons) = SharedRb::<Heap<f64>>::new(capacity).split();

                let (sx, join) = spawn_owned_noise_stream(move || {
                    let prod = move |sample: &[f64]| buffer_prod.push_slice(sample);

                    let boxed: Box<ProdType> = Box::new(prod);

                    boxed
                })?;

                (sx, join, buffer_cons)
            };

        let output_sample_rate = output_default_cfg.sample_rate().0 as f64;
        *self.sample_rate.write() = Some(output_sample_rate);
        let subnet = self.create_network(output_sample_rate);
        let mut net = Net::new(1, output_channels);
        net.set_sample_rate(output_sample_rate);

        let main_node_id = net.push(Box::new(subnet));

        // Insert smoothed gain after the main node
        let gain_param = shared(1.0f32);
        let gain_id = net.push(Box::new(
            ((var(&gain_param) >> follow(FOLLOW_RESPONSE_SECS)) * pass())
                | ((var(&gain_param) >> follow(FOLLOW_RESPONSE_SECS)) * pass()),
        ));
        net.pipe_all(main_node_id, gain_id);
        net.pipe_input(main_node_id);
        net.pipe_output(gain_id);

        net.allocate();
        net.check();

        // Split (retain front-end for later mutation).
        let mut backend = net.backend();
        let front = net;

        // Prepare CPAL stream config
        let stream_cfg: cpal::StreamConfig = output_default_cfg.clone().into();

        // Store front-end & primary node id.
        {
            *self.dsp_net_frontend.write() = Some(front);
            *self.dsp_primary_node_id.write() = Some(main_node_id);
            *self.gain_param.write() = Some(gain_param.clone());
        }

        // Create and start the stream in the dedicated owner thread.
        let (tx, handle) = spawn_owned_output_stream(
            output_device,
            output_default_cfg,
            stream_cfg,
            output_channels,
            move || {
                // Move the activator consumer into the generator closure scope.
                let mut activator_cons = activator_cons;

                let mut in_sample = [0_f32];
                let mut out_sample = [0_f32, 0_f32];

                // Ephemeral pop_iter per tick: borrow only within this call, avoiding non-Send references.
                let next_value = move || {
                    let mut tmp = [0.0f64; 1];
                    if activator_cons.pop_slice(&mut tmp) > 0 {
                        in_sample[0] = tmp[0] as f32;
                    }
                    backend.tick(&in_sample, &mut out_sample);
                    (out_sample[0], out_sample[1])
                };

                let boxed: Box<GenType> = Box::new(next_value);
                boxed
            },
        )?;
        {
            *self.control_tx.write() = Some(tx);
            *self.join.write() = Some(handle);
            *self.activation_sender.write() = Some(act_sx);
            *self.activation_thread.write() = Some(act_handle);
        }

        *self.playing.write() = true;

        Ok(true)
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

        self.fade_out();

        let tx_opt = self.control_tx.write().take();
        if let Some(tx) = tx_opt {
            let (ack_tx, ack_rx) = mpsc::channel();
            tx.send(Control::Shutdown(ack_tx))
                .map_err(|_| ControlError::ChannelSend {
                    op: "shutdown".into(),
                })?;
            match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
                Ok(ControlInvocationResult::Ok(_)) => {}
                Ok(ControlInvocationResult::Err(e)) => {
                    return Err(InstrumentError::BuildStream { detail: e }.into())
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

        // Shutdown activation system
        if let Some(act_tx) = self.activation_sender.write().take() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if act_tx.send(Control::Shutdown(ack_tx)).is_ok() {
                // Wait for activation thread ack, but don't error if it fails
                let _ = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS));
            }
        }

        *self.join.write() = None;
        *self.activation_thread.write() = None;

        self.dsp_net_frontend.write().take();
        self.dsp_primary_node_id.write().take();
        self.gain_param.write().take();

        // Clear stored handles
        {
            self.activation_snoops.write().clear();
            self.output_snoops.write().clear();
            self.siren_controls.write().clear();
            self.band_controls.write().clear();
        }

        Ok(true)
    }

    fn fade_out(&self) {
        // Extract param before sleeping
        let param = self.gain_param.read().as_ref().cloned();
        if let Some(p) = param {
            p.set(0.0);
        }
        std::thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    fn fade_in(&self) {
        // Extract param before sleeping
        let param = self.gain_param.read().as_ref().cloned();
        if let Some(p) = param {
            p.set(1.0);
        }
        std::thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    pub fn playing(&self) -> bool {
        *self.playing.read()
    }

    pub fn activation_source(&self) -> u8 {
        (*self.activation_source.read()).into()
    }

    pub fn set_activation_source(&self, requested: ActivationSource) -> AppResult<bool> {
        // Narrow lock scope - check and update in minimal scope
        let needs_update = {
            let mut src = self.activation_source.write();
            if *src != requested {
                *src = requested;
                true
            } else {
                false
            }
        };

        if needs_update {
            // recreate system with the selected activation source
            if self.playing() {
                _ = self.stop_playback()?;
                _ = self.start_playback()?;
            }
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

        self.fade_out();

        let Some(tx) = self.control_tx.read().as_ref().cloned() else {
            return Err(ControlError::BackendMissing { op: "pause".into() }.into());
        };
        let (ack_tx, ack_rx) = mpsc::channel();
        tx.send(Control::Pause(ack_tx))
            .map_err(|_| ControlError::ChannelSend { op: "pause".into() })?;
        match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
            Ok(ControlInvocationResult::Ok(_)) => Ok(true),
            Ok(ControlInvocationResult::Err(e)) => {
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
            Ok(ControlInvocationResult::Ok(_)) => {
                self.fade_in();
                Ok(true)
            }
            Ok(ControlInvocationResult::Err(e)) => {
                Err(ControlError::BuildStream { detail: e }.into())
            }
            Err(_) => Err(ControlError::AckTimeout {
                op: "resume".into(),
            }
            .into()),
        }
    }

    pub fn set_is_dark(&self, is_dark: bool) -> common::error::Result<()> {
        // Narrow lock scope - derive config outside of lock
        let new_config = {
            let mut layout = self.layout.write();
            layout.scale = if is_dark {
                common::instrument::Scale::In
            } else {
                common::instrument::Scale::Yo
            };
            common::instrument::Config::try_from(*layout)?
        };

        // Separate lock scope for config update
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
        // Narrow lock scope - update layout and derive config
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
                "Created new config for size change (len={}): {:#?}",
                config.0.len(),
                *config
            );
        }
        self.update_primary_node();
        Ok(())
    }

    pub fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        let key = NodeKey(group as u8, key as u8);

        let mut out = Vec::new();
        let mut snoops = self.output_snoops.write();
        if let Some(snoop) = snoops.get_mut(&key) {
            snoop.update();
            let cap = snoop.capacity();
            out.reserve(cap + 2);
            for rev in (0..cap).rev() {
                out.push(snoop.at(rev));
            }
        }
        out
    }

    pub fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        let layout = self.layout();
        let num_groups = layout.num_groups.get() as usize;
        let keys_per_group = layout.num_keys_per_group.get() as usize;

        let mut result = Vec::with_capacity(num_groups * keys_per_group);
        let mut snoops = self.output_snoops.write();

        for g in 0..num_groups {
            for k in 0..keys_per_group {
                let key = NodeKey(g as u8, k as u8);
                if let Some(snoop) = snoops.get_mut(&key) {
                    snoop.update();
                    let cap = snoop.capacity();
                    let mut samples = Vec::with_capacity(cap + 2);
                    for rev in (0..cap).rev() {
                        samples.push(snoop.at(rev));
                    }
                    result.push((g as u8, k as u8, samples));
                }
            }
        }

        result
    }
}
