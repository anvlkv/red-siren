use std::{
    collections::HashMap,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use common::tuner::Config as TunerConfig;
use common::{
    error::{ControlError, InstrumentError, Result},
    NodeKey,
};
use cpal::traits::{DeviceTrait, HostTrait};
use fundsp::hacker32::prelude::*;
use parking_lot::RwLock;
use ringbuf::{
    storage::Heap,
    traits::{Consumer, Producer, Split},
    SharedRb,
};

use crate::rt::{ActivationSource, StreamController};

use super::stream::{
    spawn_owned_input_stream, spawn_owned_noise_stream, spawn_owned_output_stream, Control,
    ControlInvocationResult, GenType, ProdType,
};

const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;
const FADE_DURATION_MS: u64 = 120;
const FOLLOW_RESPONSE_SECS: f32 = 0.01;
const BUFFER_DURATION_MS: u64 = 100;

/// CPAL-backed stream controller implementing audio I/O and DSP graph
/// lifecycle. The higher-level runtime facade instantiates this when the
/// `rt_cpal` feature is enabled.
struct CpalController {
    // DSP frontend and node references
    dsp_net_frontend: RwLock<Option<Net>>,
    dsp_primary_node_id: RwLock<Option<NodeId>>,
    sample_rate: RwLock<Option<f64>>,
    gain_param: RwLock<Option<Shared>>,

    // Stream control
    control_tx: RwLock<Option<Sender<Control>>>,
    output_thread: RwLock<Option<thread::JoinHandle<()>>>,

    // Activation system (input/noise)
    activation_sender: RwLock<Option<Sender<Control>>>,
    activation_thread: RwLock<Option<thread::JoinHandle<()>>>,

    // Per-string data taps and controls
    activation_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    output_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    band_controls: RwLock<HashMap<NodeKey, Shared>>,

    // Last known state for restarts
    last_layout: RwLock<InstrumentLayout>,
    last_config: RwLock<InstrumentConfig>,
    last_source: RwLock<ActivationSource>,
    last_tuner_config: RwLock<TunerConfig>,
}

impl Default for CpalController {
    fn default() -> Self {
        Self {
            dsp_net_frontend: RwLock::new(None),
            dsp_primary_node_id: RwLock::new(None),
            sample_rate: RwLock::new(None),
            gain_param: RwLock::new(None),
            control_tx: RwLock::new(None),
            output_thread: RwLock::new(None),
            activation_sender: RwLock::new(None),
            activation_thread: RwLock::new(None),
            activation_snoops: RwLock::new(HashMap::new()),
            output_snoops: RwLock::new(HashMap::new()),
            band_controls: RwLock::new(HashMap::new()),
            last_layout: RwLock::new(InstrumentLayout::default()),
            last_config: RwLock::new(InstrumentConfig::default()),
            last_source: RwLock::new(ActivationSource::default()),
            last_tuner_config: RwLock::new(TunerConfig::default()),
        }
    }
}

impl CpalController {
    fn create_network(
        &self,
        config: &InstrumentConfig,
        tuner_config: &TunerConfig,
        sample_rate: f64,
    ) -> Net {
        // For now hard-code (1 input, 2 outputs). Multi-channel path to come.
        let mut net = Net::new(1, 2);
        net.set_sample_rate(sample_rate);

        // Build output system graph & retrieve handles.
        let node_handles = crate::create_output_system(config, &mut net, 2);

        let mut siren_controls = HashMap::<NodeKey, Shared>::new();
        let mut band_controls = HashMap::<NodeKey, Shared>::new();

        // Store node handle artifacts (activation/output snoops, control vars).
        {
            let mut activation_snoops = self.activation_snoops.write();
            let mut output_snoops = self.output_snoops.write();
            let mut stored_band_controls = self.band_controls.write();

            activation_snoops.clear();
            output_snoops.clear();
            stored_band_controls.clear();

            for handle in node_handles {
                activation_snoops.insert(handle.key, handle.activation_snoop);
                output_snoops.insert(handle.key, handle.output_snoop);
                siren_controls.insert(handle.key, handle.siren_control);
                band_controls.insert(handle.key, handle.band_control.clone());
                stored_band_controls.insert(handle.key, handle.band_control);
            }
        }

        crate::create_input_system(tuner_config, &mut net, siren_controls);

        net.allocate();
        log::debug!("created network: {}", net.display());
        net
    }

    fn update_primary_node(&self, config: &InstrumentConfig, tuner_config: &TunerConfig) {
        let primary_id = match *self.dsp_primary_node_id.read() {
            Some(id) => id,
            None => {
                log::warn!("No primary node id to update");
                return;
            }
        };

        let sample_rate = self.sample_rate.read().unwrap_or(44100.0);
        let new_node = self.create_network(config, tuner_config, sample_rate);

        let mut guard = self.dsp_net_frontend.write();
        let Some(net) = guard.as_mut() else {
            log::warn!("No DSP network frontend to update");
            return;
        };

        net.crossfade(primary_id, Fade::Smooth, 0.3, Box::new(new_node));
        net.check();
        net.commit();
    }

    fn fade_out(&self) {
        if let Some(p) = self.gain_param.read().as_ref().cloned() {
            p.set(0.0);
        }
        thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    fn fade_in(&self) {
        if let Some(p) = self.gain_param.read().as_ref().cloned() {
            p.set(1.0);
        }
        thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    fn shutdown_streams(&self) -> Result<()> {
        // Output stream shutdown
        if let Some(tx) = self.control_tx.write().take() {
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
        }

        // Activation stream shutdown
        if let Some(act_tx) = self.activation_sender.write().take() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if act_tx.send(Control::Shutdown(ack_tx)).is_ok() {
                let _ = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS));
            }
        }

        // Release thread handles
        *self.output_thread.write() = None;
        *self.activation_thread.write() = None;

        // Clear DSP & handles
        self.dsp_net_frontend.write().take();
        self.dsp_primary_node_id.write().take();
        self.gain_param.write().take();
        self.activation_snoops.write().clear();
        self.output_snoops.write().clear();

        Ok(())
    }

    /// Set band control value for a specific node
    pub fn set_band_control(&self, key: NodeKey, value: f32) -> Result<()> {
        let band_controls = self.band_controls.read();
        if let Some(control) = band_controls.get(&key) {
            control.set_value(value);
            Ok(())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    /// Set band control value for a specific node
    pub fn get_band_control(&self, key: NodeKey) -> Result<f32> {
        let band_controls = self.band_controls.read();
        if let Some(control) = band_controls.get(&key) {
            Ok(control.value())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }
}

impl StreamController for CpalController {
    fn start(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        source: ActivationSource,
        tuner_config: &TunerConfig,
    ) -> Result<()> {
        log::trace!("CpalController.start: begin with source={:?}", source);
        // Snapshot for potential restarts.
        {
            *self.last_layout.write() = *layout;
            *self.last_config.write() = config.clone();
            *self.last_source.write() = source;
            *self.last_tuner_config.write() = tuner_config.clone();
        }

        let host = cpal::default_host();
        log::trace!(
            "CpalController.start: using CPAL host: {}",
            host.id().name()
        );
        let output_device = host
            .default_output_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;
        let output_device_name = output_device
            .name()
            .unwrap_or_else(|e| format!("(error getting name: {e})"));
        log::trace!(
            "CpalController.start: default output device: {}",
            output_device_name
        );
        let output_default_cfg = output_device
            .default_output_config()
            .map_err(|_| InstrumentError::OutputConfigUnavailable)?;
        log::trace!(
            "CpalController.start: output default cfg: format={:?}, channels={}, sample_rate={} Hz, buffer={:?}",
            output_default_cfg.sample_format(),
            output_default_cfg.channels(),
            output_default_cfg.sample_rate().0,
            output_default_cfg.buffer_size(),
        );

        // Currently support up to stereo.
        let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;
        log::trace!(
            "CpalController.start: using {} output channels",
            output_channels
        );

        // Prepare activation ring buffer & activation source thread.
        log::trace!(
            "CpalController.start: selecting activation source branch: {:?}",
            source
        );
        let (act_sx, act_handle, activator_cons) = if matches!(source, ActivationSource::Mic) {
            let input_device = host
                .default_input_device()
                .ok_or(InstrumentError::DeviceUnavailable)?;
            let input_device_name = input_device
                .name()
                .unwrap_or_else(|e| format!("(error getting name: {e})"));
            log::trace!(
                "CpalController.start: default input device: {}",
                input_device_name
            );
            let input_default_cfg = input_device
                .default_input_config()
                .map_err(|_| InstrumentError::InputConfigUnavailable)?;
            log::trace!(
                "CpalController.start: input default cfg: format={:?}, channels={}, sample_rate={} Hz, buffer={:?}",
                input_default_cfg.sample_format(),
                input_default_cfg.channels(),
                input_default_cfg.sample_rate().0,
                input_default_cfg.buffer_size(),
            );

            log::trace!("CpalController.start: using Mic activation");
            let stream_cfg: cpal::StreamConfig = input_default_cfg.clone().into();
            log::trace!(
                "CpalController.start: input stream_cfg: channels={}, sample_rate={} Hz",
                stream_cfg.channels,
                stream_cfg.sample_rate.0
            );
            let channels_in = stream_cfg.channels as usize;

            let samples_per_ms = stream_cfg.sample_rate.0 as f64 / 1000.0;
            let cap_samples = ((samples_per_ms * BUFFER_DURATION_MS as f64).ceil() as usize)
                .saturating_mul(channels_in);
            let capacity = cap_samples.next_power_of_two();

            let (mut buffer_prod, buffer_cons) = SharedRb::<Heap<f64>>::new(capacity).split();

            let (sx, join) =
                spawn_owned_input_stream(input_device, input_default_cfg, stream_cfg, move || {
                    // Downmix to mono when pushing into ring buffer.
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
                })?;

            (sx, join, buffer_cons)
        } else {
            // Entropy / noise activation
            log::trace!("CpalController.start: using Noise activation");
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

        // Create primary subnet & top-level net.
        let output_sample_rate = output_default_cfg.sample_rate().0 as f64;
        *self.sample_rate.write() = Some(output_sample_rate);
        let subnet = self.create_network(config, tuner_config, output_sample_rate);
        let mut net = Net::new(1, output_channels);
        net.set_sample_rate(output_sample_rate);

        let main_node_id = net.push(Box::new(subnet));

        // Insert smoothed gain after main node for fade in/out.
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

        // Split -> backend tick side & retained frontend mutation side.
        let mut backend = net.backend();
        let front = net;

        // Prepare CPAL output stream config.
        let stream_cfg: cpal::StreamConfig = output_default_cfg.clone().into();

        // Store references
        {
            *self.dsp_net_frontend.write() = Some(front);
            *self.dsp_primary_node_id.write() = Some(main_node_id);
            *self.gain_param.write() = Some(gain_param.clone());
        }

        // Spawn output stream owner.
        let (tx, handle) = spawn_owned_output_stream(
            output_device,
            output_default_cfg,
            stream_cfg,
            output_channels,
            move || {
                // Activation consumer captured inside closure.
                let mut activator_cons = activator_cons;
                let mut in_sample = [0_f32];
                let mut out_sample = [0_f32, 0_f32];
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

        // Persist thread / control handles.
        {
            *self.control_tx.write() = Some(tx);
            *self.output_thread.write() = Some(handle);
            *self.activation_sender.write() = Some(act_sx);
            *self.activation_thread.write() = Some(act_handle);
        }

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        log::trace!("CpalController.stop: begin");
        self.fade_out();
        let res = self.shutdown_streams();
        if res.is_ok() {
            log::trace!("CpalController.stop: shutdown_streams Ok");
        } else {
            log::error!("CpalController.stop: shutdown_streams Err");
        }
        res
    }

    fn pause(&self) -> Result<()> {
        log::trace!("CpalController.pause: begin");
        self.fade_out();
        let Some(tx) = self.control_tx.read().as_ref().cloned() else {
            return Err(ControlError::BackendMissing { op: "pause".into() }.into());
        };
        let (ack_tx, ack_rx) = mpsc::channel();
        tx.send(Control::Pause(ack_tx))
            .map_err(|_| ControlError::ChannelSend { op: "pause".into() })?;
        match ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS)) {
            Ok(ControlInvocationResult::Ok(_)) => Ok(()),
            Ok(ControlInvocationResult::Err(e)) => {
                Err(ControlError::BuildStream { detail: e }.into())
            }
            Err(_) => Err(ControlError::AckTimeout { op: "pause".into() }.into()),
        }
    }

    fn resume(&self) -> Result<()> {
        log::trace!("CpalController.resume: begin");
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
                Ok(())
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

    fn on_activation_source_changed(&self, source: ActivationSource) -> Result<()> {
        log::trace!(
            "CpalController.on_activation_source_changed: requested={:?}",
            source
        );
        *self.last_source.write() = source;
        let started = self.control_tx.read().is_some();
        log::trace!(
            "CpalController.on_activation_source_changed: controller started? {}",
            started
        );
        if !started {
            log::trace!("CpalController.on_activation_source_changed: backend not started; caching source and returning Ok");
            return Ok(());
        }
        // Restart streaming pipeline with new source
        log::trace!("CpalController.on_activation_source_changed: preparing to restart streams");
        let layout = *self.last_layout.read();
        let config = self.last_config.read().clone();
        let tuner_config = self.last_tuner_config.read().clone();
        log::trace!("CpalController.on_activation_source_changed: state cloned; calling stop()");
        self.stop()?;
        log::trace!(
            "CpalController.on_activation_source_changed: stop() returned Ok; calling start()"
        );
        self.start(&layout, &config, source, &tuner_config)
    }

    fn on_layout_changed(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        tuner_config: &TunerConfig,
    ) -> Result<()> {
        *self.last_layout.write() = *layout;
        *self.last_config.write() = config.clone();
        *self.last_tuner_config.write() = tuner_config.clone();
        if self.control_tx.read().is_some() {
            self.update_primary_node(config, tuner_config);
        }
        Ok(())
    }

    fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32> {
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

    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        // Use last known layout to iterate keys.
        let layout = *self.last_layout.read();
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

    fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        let key = NodeKey(group as u8, key as u8);

        let mut out = Vec::new();
        let mut snoops = self.activation_snoops.write();
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

    fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        // Use last known layout to iterate keys.
        let layout = *self.last_layout.read();
        let num_groups = layout.num_groups.get() as usize;
        let keys_per_group = layout.num_keys_per_group.get() as usize;

        let mut result = Vec::with_capacity(num_groups * keys_per_group);
        let mut snoops = self.activation_snoops.write();

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

    fn set_band_control(&self, key: NodeKey, value: f32) -> Result<()> {
        self.set_band_control(key, value)
    }

    fn get_band_control(&self, key: NodeKey) -> Result<f32> {
        self.get_band_control(key)
    }
}
/// Factory exposed to the runtime facade.
pub fn make_stream_controller() -> Result<Box<dyn StreamController + Send + Sync>> {
    Ok(Box::new(CpalController::default()))
}
