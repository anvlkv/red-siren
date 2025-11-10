use std::{
    collections::HashMap,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use common::tuner::Config as TunerConfig;
use common::{
    error::TunerError,
    instrument::{Config as InstrumentConfig, Layout as InstrumentLayout},
};
use common::{
    error::{ControlError, InstrumentError, Result},
    NodeKey,
};
use cpal::traits::{DeviceTrait, HostTrait};
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::thingbuf::ThingBuf;
use parking_lot::RwLock;

#[cfg(feature = "editor")]
use crate::system::values::{FineTunedSharedValues, FineTunedValues};
use crate::{
    rt::{cpal::stream::spawn_owned_input_stream, AudioRuntime, ExcitementSource},
    util::S,
    SensorHandles,
};

use super::audio_session;
use super::stream::{
    spawn_owned_output_stream, Control, ControlInvocationResult, GenType, ProdType,
};

const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;
const FADE_DURATION_MS: u64 = 120;
const FOLLOW_RESPONSE_SECS: f32 = 0.01;
const BUFFER_DURATION_MS: u64 = 100;
const SPECTRUM_BUFFER_CAPACITY: usize = 2;

/// CPAL-backed stream controller implementing audio I/O and DSP graph
/// lifecycle. The higher-level runtime facade instantiates this when the
/// `rt_cpal` feature is enabled.
struct CpalController {
    // DSP frontend and node references
    dsp_net_frontend: RwLock<Option<Net>>,
    dsp_primary_node_id: RwLock<Option<NodeId>>,
    sample_rate: RwLock<Option<f64>>,
    gain_param: RwLock<Option<Shared>>,
    tuner_tap_gain_param: RwLock<Option<Shared>>,

    // Fine-tuned values for editor mode
    #[cfg(feature = "editor")]
    fine_tuned_shared_values: RwLock<FineTunedSharedValues>,

    // Stream control
    control_tx: RwLock<Option<Sender<Control>>>,
    output_thread: RwLock<Option<thread::JoinHandle<()>>>,

    // Excitement system (input/noise)
    input_sender: RwLock<Option<Sender<Control>>>,
    input_thread: RwLock<Option<thread::JoinHandle<()>>>,

    // Per-string data taps and controls
    excitement_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    output_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    band_controls: RwLock<HashMap<NodeKey, Shared>>,
    key_controls: RwLock<HashMap<NodeKey, Shared>>,
    sensor_controls: RwLock<HashMap<NodeKey, SensorHandles>>,

    // Spectrum data tap
    spectrum_data_thb: crate::system::input::analyzer::SpectrumBuffer,
    siren_excitements: RwLock<HashMap<NodeKey, Var>>,

    // Last known state for restarts
    last_layout: RwLock<InstrumentLayout>,
    last_config: RwLock<InstrumentConfig>,
    last_source: RwLock<ExcitementSource>,
    last_tuner_config: RwLock<TunerConfig>,

    // devices
    output_device: RwLock<Option<cpal::Device>>,
    input_device: RwLock<Option<cpal::Device>>,

    // operation
    tuner_only_mode: RwLock<bool>,
    is_batch_processing: Arc<RwLock<bool>>,
}

impl Default for CpalController {
    fn default() -> Self {
        Self {
            dsp_net_frontend: RwLock::new(None),
            dsp_primary_node_id: RwLock::new(None),
            sample_rate: RwLock::new(None),
            gain_param: RwLock::new(None),
            tuner_tap_gain_param: RwLock::new(None),
            #[cfg(feature = "editor")]
            fine_tuned_shared_values: RwLock::new(FineTunedSharedValues::default()),
            control_tx: RwLock::new(None),
            output_thread: RwLock::new(None),
            input_sender: RwLock::new(None),
            input_thread: RwLock::new(None),
            excitement_snoops: RwLock::new(HashMap::new()),
            output_snoops: RwLock::new(HashMap::new()),
            band_controls: RwLock::new(HashMap::new()),
            key_controls: RwLock::new(HashMap::new()),
            sensor_controls: RwLock::new(HashMap::new()),
            spectrum_data_thb: Arc::new(ThingBuf::new(SPECTRUM_BUFFER_CAPACITY)),
            siren_excitements: RwLock::new(HashMap::new()),
            last_layout: RwLock::new(InstrumentLayout::default()),
            last_config: RwLock::new(InstrumentConfig::default()),
            last_source: RwLock::new(ExcitementSource::default()),
            last_tuner_config: RwLock::new(TunerConfig::default()),
            output_device: RwLock::new(None),
            input_device: RwLock::new(None),
            tuner_only_mode: RwLock::new(false),
            is_batch_processing: Arc::new(RwLock::new(false)),
        }
    }
}

impl CpalController {
    fn output_device(&self) -> Option<cpal::Device> {
        { self.output_device.read().clone() }.or_else(|| {
            let host = cpal::default_host();
            log::trace!("using CPAL host: {}", host.id().name());
            let device = host.default_output_device();

            {
                *self.output_device.write() = device.clone();
            }

            device
        })
    }

    fn sample_rate(&self) -> f64 {
        { *self.sample_rate.read() }
            .or_else(|| {
                self.output_device()
                    .and_then(|d| d.default_output_config().ok())
                    .map(|c| {
                        let sample_rate = c.sample_rate().0;
                        {
                            *self.sample_rate.write() = Some(sample_rate as f64);
                        }
                        sample_rate as f64
                    })
            })
            .unwrap_or(44100.0)
    }

    fn input_device(&self) -> Option<cpal::Device> {
        { self.input_device.read().clone() }
            .or_else(|| {
                let device = self.output_device().filter(|d| d.supports_input());

                {
                    *self.input_device.write() = device.clone();
                }

                device
            })
            .or_else(|| {
                let host = cpal::default_host();
                log::trace!("using CPAL host: {}", host.id().name());
                let device = host.default_input_device();

                {
                    *self.input_device.write() = device.clone();
                }

                device
            })
    }

    fn start_input_stream(&self) -> Result<Arc<ThingBuf<S>>> {
        let input_device = self
            .input_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;

        let input_device_name = input_device
            .name()
            .unwrap_or_else(|e| format!("(error getting name: {e})"));
        log::debug!("input device: {}", input_device_name);
        let input_default_cfg = input_device
            .default_input_config()
            .map_err(|_| InstrumentError::InputConfigUnavailable)?;

        // Get sample rate from InputStreamManager or use output rate
        let input_sr = input_default_cfg.sample_rate().0 as f64;

        let samples_per_ms = input_sr / 1000.0;
        let cap_samples = (samples_per_ms * BUFFER_DURATION_MS as f64).ceil() as usize;
        let capacity = cap_samples.next_power_of_two();

        let thb = Arc::new(ThingBuf::<S>::new(capacity));

        let stream_cfg = input_default_cfg.config();

        let prod = thb.clone();

        let (input_sx, input_handle) =
            spawn_owned_input_stream(input_device, input_default_cfg, stream_cfg, move || {
                Box::new(move |sample: &S| {
                    if prod.push(*sample).is_err() {
                        _ = prod.pop();
                        _ = prod.push(*sample);
                    }
                }) as Box<ProdType>
            })?;

        // Persist input thread / control handles.
        {
            *self.input_sender.write() = Some(input_sx);
            *self.input_thread.write() = Some(input_handle);
        }

        Ok(thb)
    }

    fn start_output_stream(
        &self,
        input_buffer: Option<Arc<ThingBuf<S>>>,
        backend: NetBackend,
    ) -> Result<()> {
        let output_device = self
            .output_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;

        let output_default_cfg = output_device
            .default_output_config()
            .map_err(|_| InstrumentError::OutputConfigUnavailable)?;

        let stream_cfg: cpal::StreamConfig = output_default_cfg.clone().into();

        let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;

        let is_batch_processing = self.is_batch_processing.clone();

        // Spawn output stream owner.
        let (tx, handle) = spawn_owned_output_stream(
            output_device,
            output_default_cfg,
            stream_cfg,
            output_channels,
            move || {
                // Excitement consumer captured inside closure.
                let input_buffer = input_buffer.clone();
                let mut backend = BigBlockAdapter::new(Box::new(backend));
                let next_value = move |batch: Option<(&mut [&mut [f32]], usize)>| {
                    if let Some((output, size)) = batch {
                        let mut input = vec![0.0; size];
                        let mut sample_i = 0;
                        while let Some(sample) = input_buffer
                            .as_ref()
                            .take_if(|_| sample_i < size)
                            .and_then(|a| a.pop())
                        {
                            input[sample_i] = sample as f32;
                            sample_i += 1;
                        }
                        backend.process_big(size, &[&input], output);
                        if !*is_batch_processing.read() {
                            *is_batch_processing.write() = true;
                        }
                        None
                    } else {
                        if *is_batch_processing.read() {
                            *is_batch_processing.write() = false;
                        }
                        let mut in_sample = [0_f32];
                        let mut out_sample = [0_f32; 2];
                        if let Some(sample) = input_buffer.as_ref().and_then(|a| a.pop()) {
                            in_sample[0] = sample as f32;
                        }
                        backend.tick(&in_sample, &mut out_sample);
                        Some((out_sample[0], out_sample[1]))
                    }
                };
                let boxed: Box<GenType> = Box::new(next_value);
                boxed
            },
        )?;

        // Persist output thread / control handles.
        {
            *self.control_tx.write() = Some(tx);
            *self.output_thread.write() = Some(handle);
        }

        Ok(())
    }

    fn create_main_network(&self, output_channels: usize, subnet: Net) -> Net {
        let mut net = Net::new(1, output_channels);

        let main_node_id = net.push(Box::new(subnet));

        // Insert smoothed gain after main node for fade in/out.
        let gain_param = shared(1.0f32);
        let tuner_tap_gain = shared(0.0f32);
        let gain_id = if output_channels == 2 {
            net.push(Box::new(
                (pass() | pass() | (pass() >> delay(0.25)))
                    >> (((var(&gain_param) >> follow(FOLLOW_RESPONSE_SECS)) * pass())
                        | ((var(&gain_param) >> follow(FOLLOW_RESPONSE_SECS)) * pass())
                        | ((pass() * var(&tuner_tap_gain)) >> split::<U2>()))
                    >> (pass() | reverse::<U2>() | pass())
                    >> (join::<U2>() | join::<U2>()),
            ))
        } else {
            net.push(Box::new(
                (join::<U2>() | (pass() >> delay(0.25)))
                    >> (((var(&gain_param) >> follow(FOLLOW_RESPONSE_SECS)) * pass())
                        | (pass() * var(&tuner_tap_gain)))
                    >> join::<U2>(),
            ))
        };
        net.pipe_all(main_node_id, gain_id);
        net.pipe_input(main_node_id);
        net.pipe_output(gain_id);

        net.set_sample_rate(self.sample_rate());
        net.allocate();
        net.check();

        {
            *self.gain_param.write() = Some(gain_param);
            *self.tuner_tap_gain_param.write() = Some(tuner_tap_gain);
            *self.dsp_primary_node_id.write() = Some(main_node_id);
            log::trace!("stored primary node id and gain params");
        }

        net
    }

    fn create_tuner_only_network(&self, tuner_config: &TunerConfig) -> Net {
        log::info!("Creating tuner only network.",);

        let mut net = Net::new(1, 3);

        let stub = net.push(Box::new(constant(0.0) | constant(0.0)));
        net.pipe_output(stub);

        let siren_controls_stub = HashMap::<NodeKey, Shared>::from_iter(
            tuner_config
                .sensor_data
                .iter()
                .map(|s| (s.key, shared(0.0))),
        );

        {
            *self.siren_excitements.write() = HashMap::from_iter(
                siren_controls_stub
                    .iter()
                    .map(|(key, shared)| (*key, Var::new(shared))),
            );
        }

        // Initialize fine-tuned values if in editor mode
        #[cfg(feature = "editor")]
        let fine_tuned_values = {
            let shared_values_lock = self.fine_tuned_shared_values.read();

            FineTunedValues::new(&shared_values_lock)
        };

        let handles = crate::create_input_system(
            tuner_config,
            &mut net,
            siren_controls_stub,
            ExcitementSource::Mic,
            &self.spectrum_data_thb,
            2,
            #[cfg(feature = "editor")]
            &fine_tuned_values,
        );

        {
            log::trace!("Storing {} sensor controls", handles.len());
            *self.sensor_controls.write() =
                HashMap::from_iter(handles.into_iter().map(|h| (h.key, h)));
            log::trace!("setting tuner_only_mode to true");
            *self.tuner_only_mode.write() = true;
            log::trace!("stored sensor controls");
        }

        net.set_sample_rate(self.sample_rate());
        net.allocate();
        net.check();

        log::debug!("created network: {}", net.display());
        net
    }

    fn create_instrument_network(
        &self,
        config: &InstrumentConfig,
        tuner_config: &TunerConfig,
        source: ExcitementSource,
    ) -> Net {
        log::info!(
            "Creating network with {} groups, {} keys per group",
            config.num_groups(),
            config.0.first().map(|g| g.nodes.len()).unwrap_or(0)
        );

        let mut net = Net::new(1, 3);

        // Preserve old control values before clearing
        let old_band_values = {
            let stored_band_controls = self.band_controls.read();
            stored_band_controls
                .iter()
                .map(|(k, v)| (*k, v.value()))
                .collect::<HashMap<NodeKey, f32>>()
        };

        let old_key_values = {
            let stored_key_controls = self.key_controls.read();
            stored_key_controls
                .iter()
                .map(|(k, v)| (*k, v.value()))
                .collect::<HashMap<NodeKey, f32>>()
        };

        // Initialize fine-tuned values if in editor mode
        #[cfg(feature = "editor")]
        let fine_tuned_values = {
            let shared_values_lock = self.fine_tuned_shared_values.read();

            FineTunedValues::new(&shared_values_lock)
        };

        // Build output system graph & retrieve handles.
        let node_handles = crate::create_output_system(
            config,
            &mut net,
            2,
            #[cfg(feature = "editor")]
            &fine_tuned_values,
        );

        let mut siren_controls = HashMap::<NodeKey, Shared>::new();

        // Store node handle artifacts (excitement/output snoops, control vars).
        {
            let mut excitement_snoops = self.excitement_snoops.write();
            let mut output_snoops = self.output_snoops.write();
            let mut stored_band_controls = self.band_controls.write();
            let mut stored_key_controls = self.key_controls.write();

            excitement_snoops.clear();
            output_snoops.clear();
            stored_band_controls.clear();
            stored_key_controls.clear();

            for handle in node_handles {
                excitement_snoops.insert(handle.key, handle.excitement_snoop);
                output_snoops.insert(handle.key, handle.output_snoop);
                siren_controls.insert(handle.key, handle.siren_control);
                stored_band_controls.insert(handle.key, handle.band_control);
                stored_key_controls.insert(handle.key, handle.key_control);
            }

            // Restore old band control values after insertion
            for (key, old_value) in old_band_values {
                if let Some(control) = stored_band_controls.get(&key) {
                    control.set_value(old_value);
                    log::debug!("Restored band control value for {:?}: {}", key, old_value);
                }
            }

            // Restore old key control values after insertion
            for (key, old_value) in old_key_values {
                if let Some(control) = stored_key_controls.get(&key) {
                    control.set_value(old_value);
                    log::debug!("Restored key control value for {:?}: {}", key, old_value);
                }
            }
        }

        log::info!(
            "Created {} siren controls for input system",
            siren_controls.len()
        );

        {
            *self.siren_excitements.write() = HashMap::from_iter(
                siren_controls
                    .iter()
                    .map(|(key, shared)| (*key, Var::new(shared))),
            );
        }

        let handles = crate::create_input_system(
            tuner_config,
            &mut net,
            siren_controls,
            source,
            &self.spectrum_data_thb,
            2,
            #[cfg(feature = "editor")]
            &fine_tuned_values,
        );

        {
            *self.sensor_controls.write() =
                HashMap::from_iter(handles.into_iter().map(|h| (h.key, h)));
            *self.tuner_only_mode.write() = false;
            log::trace!("stored sensor controls");
        }

        net.set_sample_rate(self.sample_rate());
        net.allocate();
        net.check();

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

        log::info!("Updating primary DSP node due to state change");

        let new_node = if *self.tuner_only_mode.read() {
            self.create_tuner_only_network(tuner_config)
        } else {
            let source = *self.last_source.read();
            self.create_instrument_network(config, tuner_config, source)
        };

        let mut guard = self.dsp_net_frontend.write();
        let Some(net) = guard.as_mut() else {
            log::warn!("No DSP network frontend to update");
            return;
        };

        net.crossfade(primary_id, Fade::Smooth, 0.3, Box::new(new_node));
        net.check();
        net.commit();
        log::info!("Primary DSP node updated successfully");
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

        // Excitement stream shutdown
        if let Some(act_tx) = self.input_sender.write().take() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if act_tx.send(Control::Shutdown(ack_tx)).is_ok() {
                let _ = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS));
            }
        }

        // Release thread handles
        *self.output_thread.write() = None;
        *self.input_thread.write() = None;

        // Clear DSP & handles
        self.dsp_net_frontend.write().take();
        self.dsp_primary_node_id.write().take();
        self.gain_param.write().take();
        self.excitement_snoops.write().clear();
        self.output_snoops.write().clear();
        self.band_controls.write().clear();
        self.key_controls.write().clear();
        self.sensor_controls.write().clear();
        self.sample_rate.write().take();

        Ok(())
    }

    /// Set band control value for a specific node
    fn set_band_control(&self, key: NodeKey, value: f32) -> Result<()> {
        let band_controls = self.band_controls.read();
        if let Some(control) = band_controls.get(&key) {
            control.set_value(value);
            Ok(())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    /// Get band control value for a specific node
    fn get_band_control(&self, key: NodeKey) -> Result<f32> {
        let band_controls = self.band_controls.read();
        if let Some(control) = band_controls.get(&key) {
            Ok(control.value())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    /// Set key control value for a specific node (0.0 = false/released, 1.0 = true/pressed)
    fn set_key_control(&self, key: NodeKey, value: f32) -> Result<()> {
        let key_controls = self.key_controls.read();
        if let Some(control) = key_controls.get(&key) {
            control.set_value(value);
            Ok(())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    /// Get key control value for a specific node (0.0 = false/released, 1.0 = true/pressed)
    fn get_key_control(&self, key: NodeKey) -> Result<f32> {
        let key_controls = self.key_controls.read();
        if let Some(control) = key_controls.get(&key) {
            Ok(control.value())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }
}

impl AudioRuntime for CpalController {
    fn start(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        source: ExcitementSource,
        tuner_config: &TunerConfig,
    ) -> Result<()> {
        log::trace!("CpalController.start: begin with source={:?}", source);

        // Ensure audio session is configured (iOS)
        audio_session::ensure_configured()?;

        // Snapshot for potential restarts.
        {
            *self.last_layout.write() = *layout;
            *self.last_config.write() = config.clone();
            *self.last_source.write() = source;
            *self.last_tuner_config.write() = tuner_config.clone();
            log::trace!("sotred layout/config/source/tuner_config");
        }

        let output_device = self
            .output_device()
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

        // Prepare excitement ring buffer & excitement source thread.
        log::trace!(
            "CpalController.start: selecting excitement source branch: {:?}",
            source
        );

        let input_buffer = if matches!(source, ExcitementSource::Mic) {
            log::debug!("CpalController.start: using Mic excitement");
            Some(self.start_input_stream()?)
        } else {
            None
        };

        let subnet = self.create_instrument_network(config, tuner_config, source);

        let mut net = self.create_main_network(output_channels, subnet);

        // Split -> backend tick side & retained frontend mutation side.
        let backend = net.backend();
        let front = net;

        self.start_output_stream(input_buffer, backend)?;

        // Store references
        {
            *self.dsp_net_frontend.write() = Some(front);
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

    fn on_excitement_source_changed(&self, source: ExcitementSource) -> Result<()> {
        log::trace!(
            "CpalController.on_excitement_source_changed: requested={:?}",
            source
        );
        *self.last_source.write() = source;
        let started = self.control_tx.read().is_some();
        log::trace!(
            "CpalController.on_excitement_source_changed: controller started? {}",
            started
        );
        if !started {
            log::trace!("CpalController.on_excitement_source_changed: backend not started; caching source and returning Ok");
            return Ok(());
        }
        // Restart streaming pipeline with new source
        log::trace!("CpalController.on_excitement_source_changed: preparing to restart streams");
        let layout = *self.last_layout.read();
        let config = self.last_config.read().clone();
        let tuner_config = self.last_tuner_config.read().clone();
        log::trace!("CpalController.on_excitement_source_changed: state cloned; calling stop()");
        self.stop()?;
        log::trace!(
            "CpalController.on_excitement_source_changed: stop() returned Ok; calling start()"
        );
        self.start(&layout, &config, source, &tuner_config)
    }

    fn on_layout_changed(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        tuner_config: &TunerConfig,
    ) -> Result<()> {
        log::info!(
            "Layout changed: groups={}, keys_per_group={}",
            layout.num_groups.get(),
            layout.num_keys_per_group.get()
        );

        // Validate NodeKey consistency between configs
        let registry = layout.registry();

        // Check for invalid NodeKeys in tuner config
        let sensor_keys: std::collections::HashMap<NodeKey, ()> = tuner_config
            .sensor_data
            .iter()
            .map(|s| (s.key, ()))
            .collect();
        let invalid_sensor_keys = registry.has_invalid_keys(&sensor_keys);
        if !invalid_sensor_keys.is_empty() {
            log::warn!("Invalid sensor NodeKeys found: {:?}", invalid_sensor_keys);
        }

        *self.last_layout.write() = *layout;
        *self.last_config.write() = config.clone();
        *self.last_tuner_config.write() = tuner_config.clone();
        if self.control_tx.read().is_some() {
            self.update_primary_node(config, tuner_config);
        } else {
            log::warn!("Audio stream not running, layout change will apply on next start");
        }
        Ok(())
    }

    fn snapshot_output_snoop(&self, node_key: NodeKey) -> Vec<f32> {
        let mut out = Vec::new();
        let mut snoops = self.output_snoops.write();
        if let Some(snoop) = snoops.get_mut(&node_key) {
            snoop.update();
            let cap = snoop.capacity();
            out.reserve(cap + 2);
            for rev in (0..cap).rev() {
                out.push(snoop.at(rev));
            }
        }
        out
    }

    fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
        let layout = *self.last_layout.read();
        let registry = layout.registry();

        let mut result = Vec::with_capacity(registry.total_keys());
        let mut snoops = self.output_snoops.write();

        registry.iter_keys(|node_key| {
            if let Some(snoop) = snoops.get_mut(&node_key) {
                snoop.update();
                let cap = snoop.capacity();
                let mut samples = Vec::with_capacity(cap + 2);
                for rev in (0..cap).rev() {
                    samples.push(snoop.at(rev));
                }
                result.push((node_key, samples));
            }
        });
        result
    }

    fn snapshot_excitement_snoop(&self, node_key: NodeKey) -> Vec<f32> {
        let mut out = Vec::new();
        let mut snoops = self.excitement_snoops.write();
        if let Some(snoop) = snoops.get_mut(&node_key) {
            snoop.update();
            let cap = snoop.capacity();
            out.reserve(cap + 2);
            for rev in (0..cap).rev() {
                out.push(snoop.at(rev));
            }
        }
        out
    }

    fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
        let layout = *self.last_layout.read();
        let registry = layout.registry();

        let mut result = Vec::with_capacity(registry.total_keys());
        let mut snoops = self.excitement_snoops.write();

        registry.iter_keys(|node_key| {
            if let Some(snoop) = snoops.get_mut(&node_key) {
                snoop.update();
                let cap = snoop.capacity();
                let mut samples = Vec::with_capacity(cap + 2);
                for rev in (0..cap).rev() {
                    samples.push(snoop.at(rev));
                }
                result.push((node_key, samples));
            }
        });
        result
    }

    fn set_band_control(&self, key: NodeKey, value: f32) -> Result<()> {
        self.set_band_control(key, value)
    }

    fn get_band_control(&self, key: NodeKey) -> Result<f32> {
        self.get_band_control(key)
    }

    fn set_key_control(&self, key: NodeKey, value: f32) -> Result<()> {
        self.set_key_control(key, value)
    }

    fn get_key_control(&self, key: NodeKey) -> Result<f32> {
        self.get_key_control(key)
    }

    fn poll_tuner_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot> {
        self.spectrum_data_thb.pop().map(|d| {
            common::tuner::SpectrumSnapshot(
                d.data()
                    .iter()
                    .map(|(freq, mag)| (freq.val(), mag.val()))
                    .collect(),
            )
        })
    }

    fn poll_tuner_excitements(&self) -> Vec<(NodeKey, f32)> {
        let mut data = self
            .siren_excitements
            .read()
            .iter()
            .map(|(k, v)| (*k, v.value()))
            .collect::<Vec<_>>();
        data.sort_by_key(|(k, _)| *k);

        data
    }

    fn start_tuner_only(&self, tuner_config: &TunerConfig) -> common::error::Result<()> {
        {
            *self.last_tuner_config.write() = tuner_config.clone();
        }

        let input_stream = self.start_input_stream()?;

        let subnet = self.create_tuner_only_network(tuner_config);

        let output_device = self
            .output_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;

        let output_default_cfg = output_device
            .default_output_config()
            .map_err(|_| InstrumentError::OutputConfigUnavailable)?;

        let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;
        log::trace!(
            "CpalController.start: using {} output channels",
            output_channels
        );

        let mut net = self.create_main_network(output_channels, subnet);

        let backend = net.backend();
        let front = net;

        self.start_output_stream(Some(input_stream), backend)?;

        {
            *self.dsp_net_frontend.write() = Some(front);
        }

        Ok(())
    }

    fn start_tap_tuner_audio(&self) -> common::error::Result<()> {
        if let Some(tuner_tap_gain_param) = self.tuner_tap_gain_param.read().as_ref() {
            tuner_tap_gain_param.set_value(1.0);
            Ok(())
        } else {
            Err(TunerError::MissingParameter("tap gain".to_string()).into())
        }
    }

    fn stop_tap_tuner_audio(&self) -> common::error::Result<()> {
        if let Some(tuner_tap_gain_param) = self.tuner_tap_gain_param.read().as_ref() {
            tuner_tap_gain_param.set_value(0.0);
            Ok(())
        } else {
            Err(TunerError::MissingParameter("tap gain".to_string()).into())
        }
    }

    fn update_tuner_config(&self, new_config: &TunerConfig) -> common::error::Result<()> {
        let old_config = self.last_tuner_config.read().clone();
        {
            *self.last_tuner_config.write() = new_config.clone();
        }
        if old_config
            .sensor_data
            .iter()
            .map(|d| d.key)
            .collect::<Vec<_>>()
            != new_config
                .sensor_data
                .iter()
                .map(|d| d.key)
                .collect::<Vec<_>>()
        {
            self.update_primary_node(&self.last_config.read(), new_config);
        } else {
            let controls = self.sensor_controls.read();
            for sensor_data in &new_config.sensor_data {
                if let Some(ctrl) = controls.get(&sensor_data.key) {
                    ctrl.max_frequency.set_value(sensor_data.max_frequency);
                    ctrl.min_frequency.set_value(sensor_data.min_frequency);
                    ctrl.max_magnitude.set_value(sensor_data.max_magnitude);
                    ctrl.min_magnitude.set_value(sensor_data.min_magnitude);
                    log::info!("Updated tuner sensor control for key {:?}", sensor_data.key);
                } else {
                    return Err(TunerError::MissingParameter(format!(
                        "controls for node key: {:?}",
                        sensor_data.key
                    ))
                    .into());
                }
            }
        }

        Ok(())
    }

    fn get_sample_rate(&self) -> f64 {
        self.sample_rate()
    }

    fn is_batch_processing(&self) -> bool {
        *self.is_batch_processing.read()
    }

    #[cfg(feature = "editor")]
    fn get_finetuned_values(&self) -> Result<common::commands::edit::FineTunedValuesPayload> {
        let shared_values = self.fine_tuned_shared_values.read();
        Ok(common::commands::edit::FineTunedValuesPayload {
            siren_alpha: shared_values.siren_alpha.value(),
            siren_beta: shared_values.siren_beta.value(),
            siren_gamma: shared_values.siren_gamma.value(),
            group_q: shared_values.group_q.value(),
            group_ls_gain: shared_values.group_ls_gain.value(),
            filter_switch_follow_response_s: shared_values.filter_switch_follow_response_s.value(),
            node_follow_response_time_s: shared_values.node_follow_response_time_s.value(),
            filter_allpass_q: shared_values.filter_allpass_q.value(),
            filter_allpass_freq_ratio: shared_values.filter_allpass_freq_ratio.value(),
            filter_moog_freq_ratio: shared_values.filter_moog_freq_ratio.value(),
            filter_moog_q: shared_values.filter_moog_q.value(),
            filter_shelf_freq_ratio: shared_values.filter_shelf_freq_ratio.value(),
            filter_shelf_q: shared_values.filter_shelf_q.value(),
            filter_shelf_gain: shared_values.filter_shelf_gain.value(),
            filter_pass_freq_ratio: shared_values.filter_pass_freq_ratio.value(),
            filter_pass_q: shared_values.filter_pass_q.value(),
            node_bell_q: shared_values.node_bell_q.value(),
            node_bell_gain_db: shared_values.node_bell_gain_db.value(),
            formant_base_q: shared_values.formant_base_q.value(),
            input_ny_threshold: shared_values.input_ny_threshold.value(),
            input_ny_wet_ratio: shared_values.input_ny_wet_ratio.value(),
        })
    }

    #[cfg(feature = "editor")]
    #[allow(clippy::too_many_arguments)]
    fn set_finetuned_values(
        &self,
        payload: common::commands::edit::FineTunedValuesPayload,
    ) -> Result<()> {
        {
            let shared_values = self.fine_tuned_shared_values.write();
            shared_values.siren_alpha.set_value(payload.siren_alpha);
            shared_values.siren_beta.set_value(payload.siren_beta);
            shared_values.siren_gamma.set_value(payload.siren_gamma);
            shared_values
                .node_follow_response_time_s
                .set_value(payload.node_follow_response_time_s);
            shared_values.group_q.set_value(payload.group_q);
            shared_values.group_ls_gain.set_value(payload.group_ls_gain);
            shared_values
                .filter_switch_follow_response_s
                .set_value(payload.filter_switch_follow_response_s);
            shared_values
                .filter_allpass_q
                .set_value(payload.filter_allpass_q);
            shared_values
                .filter_allpass_freq_ratio
                .set_value(payload.filter_allpass_freq_ratio);
            shared_values.filter_moog_q.set_value(payload.filter_moog_q);
            shared_values
                .filter_moog_freq_ratio
                .set_value(payload.filter_moog_freq_ratio);
            shared_values
                .filter_shelf_freq_ratio
                .set_value(payload.filter_shelf_freq_ratio);
            shared_values
                .filter_shelf_q
                .set_value(payload.filter_shelf_q);
            shared_values
                .filter_shelf_gain
                .set_value(payload.filter_shelf_gain);
            shared_values
                .filter_pass_freq_ratio
                .set_value(payload.filter_pass_freq_ratio);
            shared_values.filter_pass_q.set_value(payload.filter_pass_q);
            shared_values.node_bell_q.set_value(payload.node_bell_q);
            shared_values
                .node_bell_gain_db
                .set_value(payload.node_bell_gain_db);
            shared_values
                .formant_base_q
                .set_value(payload.formant_base_q);
            shared_values
                .input_ny_threshold
                .set_value(payload.input_ny_threshold);
            shared_values
                .input_ny_wet_ratio
                .set_value(payload.input_ny_wet_ratio);
        }

        self.update_primary_node(&self.last_config.read(), &self.last_tuner_config.read());

        Ok(())
    }
}
/// Factory exposed to the runtime facade.
pub fn make_stream_controller() -> Result<Box<dyn AudioRuntime + Send + Sync>> {
    Ok(Box::new(CpalController::default()))
}
