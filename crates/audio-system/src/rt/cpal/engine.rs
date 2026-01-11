use std::{
    collections::HashMap,
    f32,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use common::{
    error::TunerError,
    instrument::{Config as InstrumentConfig, Layout as InstrumentLayout, Preset},
};
use common::{
    error::{ControlError, InstrumentError, Result},
    NodeKey,
};
use common::{instrument::PlaybackQuality, tuner::Config as TunerConfig};
use cpal::traits::{DeviceTrait, HostTrait};
use fundsp::prelude::*;
use fundsp::thingbuf::ThingBuf;
use parking_lot::RwLock;

#[cfg(feature = "editor")]
use crate::system::values::{FineTunedSharedValues, FineTunedValues};
use crate::{
    output_analyzer::{self, OUTPUT_ANALYZER_FFT_WINDOW_SIZE},
    quality::{PlaybackQualityGate, SampleType},
    rt::{
        cpal::stream::{playback_callback, spawn_owned_input_stream},
        AudioRuntime, ExcitementSource,
    },
    system::input::analyzer::FFT_WINDOW_SIZE,
    ExcitementControl, SensorHandles,
};

use super::audio_session;
use super::stream::{spawn_owned_output_stream, Control, ControlInvocationResult, ProdType};

const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;
const FADE_DURATION_MS: u64 = 120;
const FOLLOW_RESPONSE_SECS: f32 = FADE_DURATION_MS as f32 / 1000.0;
const INPUT_BUFFER_DURATION_MS: u64 = 20;
const INPUT_SNOOP_SIZE: usize = FFT_WINDOW_SIZE;
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
    processed_output_snoops: RwLock<Option<(Snoop, Snoop)>>,
    input_snoop: RwLock<Option<Snoop>>,
    /// min and max values
    freq_range: RwLock<(f64, f64)>,
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

    // Per-node data taps and controls
    node_excitement_snoops: RwLock<HashMap<NodeKey, (Snoop, Snoop)>>,
    node_output_snoops: RwLock<HashMap<NodeKey, Snoop>>,

    // presets
    preset: RwLock<Preset>,
    node_band_controls: RwLock<HashMap<NodeKey, Shared>>,
    node_key_controls: RwLock<HashMap<NodeKey, Shared>>,
    node_sensor_controls: RwLock<HashMap<NodeKey, SensorHandles>>,
    tuner_freq_range: Arc<(Shared, Shared)>,
    tuner_ny_threshold: Arc<Shared>,
    tuner_ny_wet_ratio: Arc<Shared>,

    // Spectrum data tap
    spectrum_data_thb: crate::system::input::analyzer::SpectrumBuffer,
    siren_excitements: RwLock<HashMap<NodeKey, ExcitementControl>>,

    // Last known state for restarts
    last_layout: RwLock<InstrumentLayout>,
    last_config: RwLock<InstrumentConfig>,
    last_source: RwLock<ExcitementSource>,
    last_tuner_config: RwLock<TunerConfig>,

    // devices
    output_device: RwLock<Option<cpal::Device>>,
    input_device: RwLock<Option<cpal::Device>>,

    // operation
    tuner_only_mode: Arc<RwLock<bool>>,
    quality_indicator: Arc<std::sync::atomic::AtomicI8>,
    quality_gate: RwLock<PlaybackQualityGate>,
    auto_quality: Arc<RwLock<bool>>,
}

impl Default for CpalController {
    fn default() -> Self {
        let default_tuner_cfg = TunerConfig::default();
        Self {
            dsp_net_frontend: RwLock::new(None),
            dsp_primary_node_id: RwLock::new(None),
            sample_rate: RwLock::new(None),
            gain_param: RwLock::new(None),
            processed_output_snoops: RwLock::new(None),
            tuner_tap_gain_param: RwLock::new(None),
            freq_range: RwLock::new((
                common::instrument::consts::SOFT_MIN_FREQ_HZ,
                common::instrument::consts::SOFT_MAX_FREQ_HZ,
            )),
            #[cfg(feature = "editor")]
            fine_tuned_shared_values: RwLock::new(FineTunedSharedValues::default()),
            input_snoop: RwLock::new(None),
            tuner_freq_range: Arc::new((shared(f32::NEG_INFINITY), shared(f32::INFINITY))),
            tuner_ny_threshold: Arc::new(shared(default_tuner_cfg.ny_threshold)),
            tuner_ny_wet_ratio: Arc::new(shared(default_tuner_cfg.ny_wet_ratio)),
            control_tx: RwLock::new(None),
            output_thread: RwLock::new(None),
            input_sender: RwLock::new(None),
            input_thread: RwLock::new(None),
            node_excitement_snoops: RwLock::new(HashMap::new()),
            node_output_snoops: RwLock::new(HashMap::new()),
            preset: RwLock::new(Preset::default()),
            node_band_controls: RwLock::new(HashMap::new()),
            node_key_controls: RwLock::new(HashMap::new()),
            node_sensor_controls: RwLock::new(HashMap::new()),
            spectrum_data_thb: Arc::new(ThingBuf::new(SPECTRUM_BUFFER_CAPACITY)),
            siren_excitements: RwLock::new(HashMap::new()),
            last_layout: RwLock::new(InstrumentLayout::default()),
            last_config: RwLock::new(InstrumentConfig::default()),
            last_source: RwLock::new(ExcitementSource::default()),
            last_tuner_config: RwLock::new(TunerConfig::default()),
            output_device: RwLock::new(None),
            input_device: RwLock::new(None),
            tuner_only_mode: Arc::new(RwLock::new(false)),
            quality_indicator: Arc::new(std::sync::atomic::AtomicI8::new(
                PlaybackQualityGate::default() as i8,
            )),
            quality_gate: RwLock::new(PlaybackQualityGate::default()),
            auto_quality: Arc::new(RwLock::new(true)),
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
                        let sample_rate = c.sample_rate();
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

    fn start_input_stream(&self) -> Result<Arc<ThingBuf<f32>>> {
        let input_device = self
            .input_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;

        let input_device_name = input_device.id().unwrap();
        log::debug!("input device: {}", input_device_name);
        // Feature-driven selection of input config; fallback to device default.
        let input_default_cfg = self
            .quality_gate
            .read()
            .select_input_config(&input_device)
            .ok_or(InstrumentError::InputConfigUnavailable)?;

        // Get sample rate from InputStreamManager or use output rate
        let input_sr = input_default_cfg.sample_rate() as f64;

        let samples_per_ms = input_sr / 1000.0;
        let cap_samples = (samples_per_ms * INPUT_BUFFER_DURATION_MS as f64).ceil() as usize;
        let capacity = cap_samples.next_power_of_two();

        let thb = Arc::new(ThingBuf::<f32>::new(capacity));

        let stream_cfg = input_default_cfg.config();

        let prod = thb.clone();

        let (input_sx, input_handle) =
            spawn_owned_input_stream(input_device, input_default_cfg, stream_cfg, move || {
                Box::new(move |sample: &f32| {
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
        input_buffer: Option<Arc<ThingBuf<f32>>>,
        backend: NetBackend,
    ) -> Result<()> {
        let output_device = self
            .output_device()
            .ok_or(InstrumentError::DeviceUnavailable)?;

        let output_default_cfg = self
            .quality_gate
            .read()
            .select_output_config(&output_device)
            .ok_or(InstrumentError::OutputConfigUnavailable)?;

        let stream_cfg: cpal::StreamConfig = output_default_cfg.clone().into();

        let output_channels = std::cmp::Ord::min(output_default_cfg.channels(), 2) as usize;

        let quality: std::sync::Arc<std::sync::atomic::AtomicI8> = self.quality_indicator.clone();
        let sr = self.sample_rate.read().map(|sr| sr as u32).unwrap_or(44100);
        let no_reset = self.tuner_only_mode.clone();
        // Create shared telemetry handle and seed sample rate.
        let telemetry = std::sync::Arc::new(parking_lot::Mutex::new(
            crate::rt::cpal::stream::telemetry::PlaybackTelemetry {
                sample_rate: sr,
                ..Default::default()
            },
        ));
        // Spawn output stream owner.
        let (tx, handle) = spawn_owned_output_stream(
            output_device,
            output_default_cfg,
            stream_cfg,
            output_channels,
            {
                let telemetry = telemetry.clone();
                let buffer_target_frames = self.quality_gate.read().buffer_size(None);
                move || {
                    playback_callback(
                        backend,
                        input_buffer,
                        quality,
                        no_reset,
                        sr,
                        buffer_target_frames as usize,
                        telemetry,
                    )
                }
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
        denormal::prevent_denormals();

        let mut net = Net::new(1, output_channels);

        let main_node_id = net.push(Box::new(subnet));

        let (processed_output_snoop_l, processed_output_snoop_backend_l) =
            snoop(OUTPUT_ANALYZER_FFT_WINDOW_SIZE);
        let (processed_output_snoop_r, processed_output_snoop_backend_r) =
            snoop(OUTPUT_ANALYZER_FFT_WINDOW_SIZE);
        // Insert smoothed gain after main node for fade in/out.
        let gain_param = shared(1.0f32);
        let tuner_tap_gain = shared(0.0f32);
        let processed_output_snoops_id = net.push(Box::new(
            processed_output_snoop_backend_l | processed_output_snoop_backend_r,
        ));
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
        net.pipe_all(main_node_id, processed_output_snoops_id);
        net.pipe_all(processed_output_snoops_id, gain_id);
        net.pipe_input(main_node_id);
        net.pipe_output(gain_id);

        net.set_sample_rate(self.sample_rate());
        net.allocate();
        net.check();

        {
            *self.gain_param.write() = Some(gain_param);
            *self.processed_output_snoops.write() =
                Some((processed_output_snoop_l, processed_output_snoop_r));
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

        let siren_controls_stub = HashMap::<NodeKey, ExcitementControl>::from_iter(
            tuner_config
                .sensor_data
                .iter()
                .map(|s| (s.key, ExcitementControl::default())),
        );

        {
            *self.siren_excitements.write() = siren_controls_stub.clone();
        }

        let snoop_be = {
            let (snoop, be) = snoop(INPUT_SNOOP_SIZE);
            *self.input_snoop.write() = Some(snoop);
            Some(be)
        };

        let handles = match self.quality_gate.read().sample_type() {
            SampleType::F32 => crate::create_input_system::<f32>(
                tuner_config,
                &mut net,
                siren_controls_stub,
                ExcitementSource::Mic,
                &self.spectrum_data_thb,
                snoop_be,
                (&self.tuner_freq_range.0, &self.tuner_freq_range.1),
                (&self.tuner_ny_threshold, &self.tuner_ny_wet_ratio),
                2,
            ),
            SampleType::F64 => crate::create_input_system::<f64>(
                tuner_config,
                &mut net,
                siren_controls_stub,
                ExcitementSource::Mic,
                &self.spectrum_data_thb,
                snoop_be,
                (&self.tuner_freq_range.0, &self.tuner_freq_range.1),
                (&self.tuner_ny_threshold, &self.tuner_ny_wet_ratio),
                2,
            ),
        };

        {
            log::trace!("Storing {} sensor controls", handles.len());
            *self.node_sensor_controls.write() =
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

        // Initialize fine-tuned values if in editor mode
        #[cfg(feature = "editor")]
        let fine_tuned_values = {
            let shared_values_lock = self.fine_tuned_shared_values.read();

            FineTunedValues::new(&shared_values_lock)
        };

        // Build output system graph & retrieve handles.
        let node_handles = match self.quality_gate.read().sample_type() {
            SampleType::F32 => crate::create_output_system::<f32>(
                config,
                &mut net,
                2,
                #[cfg(feature = "editor")]
                &fine_tuned_values,
            ),
            SampleType::F64 => crate::create_output_system::<f64>(
                config,
                &mut net,
                2,
                #[cfg(feature = "editor")]
                &fine_tuned_values,
            ),
        };

        let mut siren_controls = HashMap::<NodeKey, ExcitementControl>::new();

        // Store node handle artifacts (excitement/output snoops, control vars).
        // Build maps off-lock, snapshot preset (read)
        let preset_snapshot = { self.preset.read().clone() };

        let mut new_excitement_snoops = HashMap::new();
        let mut new_output_snoops = HashMap::new();
        let mut new_band_controls = HashMap::new();
        let mut new_key_controls = HashMap::new();

        for handle in node_handles {
            new_excitement_snoops.insert(
                handle.key,
                (handle.excitement_snoop, handle.secondary_excitement_snoop),
            );
            new_output_snoops.insert(handle.key, handle.output_snoop);
            siren_controls.insert(handle.key, handle.siren_control);

            if let Some(val) = preset_snapshot.get_band_value(&handle.key) {
                handle.band_control.set_value(val);
            }
            if let Some(val) = preset_snapshot.get_key_value(&handle.key) {
                handle.key_control.set_value(val);
            }

            new_band_controls.insert(handle.key, handle.band_control);
            new_key_controls.insert(handle.key, handle.key_control);
        }

        // Commit maps in short write sections
        *self.node_excitement_snoops.write() = new_excitement_snoops;
        *self.node_output_snoops.write() = new_output_snoops;
        *self.node_band_controls.write() = new_band_controls;
        *self.node_key_controls.write() = new_key_controls;

        log::info!(
            "Created {} siren controls for input system",
            siren_controls.len()
        );

        {
            *self.siren_excitements.write() = siren_controls.clone();
        }

        let snoop_be = match source {
            ExcitementSource::Entropy => None,
            ExcitementSource::Mic => {
                let (snoop, be) = snoop(INPUT_SNOOP_SIZE);
                *self.input_snoop.write() = Some(snoop);
                Some(be)
            }
        };

        let handles = match self.quality_gate.read().sample_type() {
            SampleType::F32 => crate::create_input_system::<f32>(
                tuner_config,
                &mut net,
                siren_controls,
                source,
                &self.spectrum_data_thb,
                snoop_be,
                (&self.tuner_freq_range.0, &self.tuner_freq_range.1),
                (&self.tuner_ny_threshold, &self.tuner_ny_wet_ratio),
                2,
            ),
            SampleType::F64 => crate::create_input_system::<f64>(
                tuner_config,
                &mut net,
                siren_controls,
                source,
                &self.spectrum_data_thb,
                snoop_be,
                (&self.tuner_freq_range.0, &self.tuner_freq_range.1),
                (&self.tuner_ny_threshold, &self.tuner_ny_wet_ratio),
                2,
            ),
        };

        {
            *self.node_sensor_controls.write() =
                HashMap::from_iter(handles.into_iter().map(|h| (h.key, h)));
            *self.tuner_only_mode.write() = false;
            log::trace!("stored sensor controls");

            *self.freq_range.write() = (config.min_frequency_hz(), config.max_frequency_hz());
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

        // Fade audio out BEFORE locking the frontend
        self.fade_out();

        // Minimize lock lifetime: only hold while mutating the net
        {
            let mut guard = self.dsp_net_frontend.write();
            let Some(net) = guard.as_mut() else {
                log::warn!("No DSP network frontend to update");
                // Fade back in even if we couldn't update
                self.fade_in();
                return;
            };

            net.replace(primary_id, Box::new(new_node));
            net.reset();
            net.check();
            net.commit();
        }

        log::info!("Primary DSP node updated successfully");

        // Fade back in AFTER lock is released
        self.fade_in();
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
        // Take thread handles
        _ = self.output_thread.write().take();
        _ = self.input_thread.write().take();

        // Take DSP
        _ = self.dsp_net_frontend.write().take();
        _ = self.dsp_primary_node_id.write().take();
        _ = self.gain_param.write().take();
        _ = self.sample_rate.write().take();

        // Clear snoops & controls
        self.node_excitement_snoops.write().clear();
        self.node_output_snoops.write().clear();
        self.node_band_controls.write().clear();
        self.node_key_controls.write().clear();
        self.node_sensor_controls.write().clear();

        // Take control channels
        let control_tx = self.control_tx.write().take();
        let input_sender = self.input_sender.write().take();

        // Output stream shutdown
        if let Some(tx) = control_tx {
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
        if let Some(act_tx) = input_sender {
            let (ack_tx, ack_rx) = mpsc::channel();
            if act_tx.send(Control::Shutdown(ack_tx)).is_ok() {
                let _ = ack_rx.recv_timeout(Duration::from_millis(CONTROL_INVOKE_TIMEOUT_MS));
            }
        }

        Ok(())
    }

    /// Set band control value for a specific node
    fn set_band_control(&self, key: NodeKey, value: f32) -> Result<()> {
        self.preset.write().set_band_value(&key, value);
        let band_controls = self.node_band_controls.read();
        if let Some(control) = band_controls.get(&key) {
            control.set_value(value);
            Ok(())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    /// Get band control value for a specific node
    fn get_band_control(&self, key: NodeKey) -> Result<f32> {
        let band_controls = self.node_band_controls.read();
        if let Some(control) = band_controls.get(&key) {
            Ok(control.value())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    /// Set key control value for a specific node (0.0 = false/released, 1.0 = true/pressed)
    fn set_key_control(&self, key: NodeKey, value: f32) -> Result<()> {
        self.preset.write().set_key_value(&key, value);
        let key_controls = self.node_key_controls.read();
        if let Some(control) = key_controls.get(&key) {
            control.set_value(value);
            Ok(())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    /// Get key control value for a specific node (0.0 = false/released, 1.0 = true/pressed)
    fn get_key_control(&self, key: NodeKey) -> Result<f32> {
        let key_controls = self.node_key_controls.read();
        if let Some(control) = key_controls.get(&key) {
            Ok(control.value())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    fn set_tuner_input_values(&self, tuner_config: &TunerConfig) {
        self.tuner_freq_range
            .0
            .set_value(tuner_config.frequency_range.0.unwrap_or(f32::NEG_INFINITY));
        self.tuner_freq_range
            .1
            .set_value(tuner_config.frequency_range.1.unwrap_or(f32::INFINITY));
        self.tuner_ny_threshold.set_value(tuner_config.ny_threshold);
        self.tuner_ny_wet_ratio.set_value(tuner_config.ny_wet_ratio);
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

        self.set_tuner_input_values(tuner_config);

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
        let output_device_name = output_device.id().unwrap();
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
            output_default_cfg.sample_rate(),
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

        self.set_tuner_input_values(tuner_config);

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
        let mut snoops = self.node_output_snoops.write();
        if let Some(snoop) = snoops.get_mut(&node_key) {
            snoop.update();
            let cap = snoop.capacity();
            out.reserve(cap + 2);
            for rev in (0..cap).rev() {
                let s = snoop.at(rev);
                if s.is_normal() || s == 0.0 {
                    out.push(snoop.at(rev));
                } else {
                    out.push(0.0);
                }
            }
        }
        out
    }

    fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
        let layout = *self.last_layout.read();
        let registry = layout.registry();

        registry
            .all_keys()
            .iter()
            .map(|&node_key| (node_key, self.snapshot_output_snoop(node_key)))
            .collect()
    }

    fn snapshot_excitement_snoop(&self, node_key: NodeKey) -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        let mut snoops = self.node_excitement_snoops.write();
        if let Some((primary, secondary)) = snoops.get_mut(&node_key) {
            primary.update();
            secondary.update();
            let cap = primary.capacity();
            out.reserve(cap + 2);
            for rev in (0..cap).rev() {
                let p = primary.at(rev);
                let s = secondary.at(rev);
                // avoid denormals in output
                if (p.is_normal() || p == 0.0) && (s.is_normal() || s == 0.0) {
                    out.push((primary.at(rev), secondary.at(rev)));
                } else {
                    out.push((0.0, 0.0));
                }
            }
        }
        out
    }

    fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)> {
        let layout = *self.last_layout.read();
        let registry = layout.registry();

        let mut result = Vec::with_capacity(registry.total_keys());

        registry.iter_keys(|node_key| {
            result.push((node_key, self.snapshot_excitement_snoop(node_key)))
        });
        result
    }

    fn snapshot_processed_output_spectrum(
        &self,
    ) -> Result<Option<crate::rt::ProcessedOutputSpectrumSnapshot>> {
        // Snapshot sample rate and frequency range using short-lived locks
        let sample_rate = match self.sample_rate.read().as_ref() {
            Some(sr) => *sr,
            None => return Ok(None),
        };
        let (min_hz, max_hz) = *self.freq_range.read();

        // Local windows to fill without holding locks during heavy processing
        let mut l_window = [0.0; OUTPUT_ANALYZER_FFT_WINDOW_SIZE];
        let mut r_window = [0.0; OUTPUT_ANALYZER_FFT_WINDOW_SIZE];

        // Pull available buffers quickly; avoid sleeping while holding locks
        let filled = {
            if let Some((l, r)) = self.processed_output_snoops.write().as_mut() {
                let mut len = 0;
                while len < OUTPUT_ANALYZER_FFT_WINDOW_SIZE {
                    if let Some((l_buffer, r_buffer)) = l.get().zip(r.get()) {
                        let remaining = OUTPUT_ANALYZER_FFT_WINDOW_SIZE - len;
                        let num_samples = std::cmp::Ord::min(l_buffer.size(), remaining);
                        for i in 0..num_samples {
                            l_window[len + i] = l_buffer.at(i);
                            r_window[len + i] = r_buffer.at(i);
                        }
                        len += num_samples;
                    } else {
                        // Not enough data available right now; bail out early
                        break;
                    }
                }
                len
            } else {
                0
            }
        };

        // If buffers were not ready, let caller try again later
        if filled < OUTPUT_ANALYZER_FFT_WINDOW_SIZE {
            return Ok(None);
        }

        // Perform analysis without holding any locks
        let left_spectrum =
            output_analyzer::analyze(l_window, sample_rate, min_hz as f32, max_hz as f32)?;
        let right_spectrum =
            output_analyzer::analyze(r_window, sample_rate, min_hz as f32, max_hz as f32)?;

        Ok(Some((left_spectrum, right_spectrum)))
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
            .map(|(k, v)| (*k, v.value::<f32>().re))
            .collect::<Vec<_>>();
        data.sort_by_key(|(k, _)| *k);

        data
    }

    fn start_tuner_only(&self, tuner_config: &TunerConfig) -> common::error::Result<()> {
        self.set_tuner_input_values(tuner_config);

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
            let controls = self.node_sensor_controls.read();
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

        self.set_tuner_input_values(new_config);

        Ok(())
    }

    fn get_sample_rate(&self) -> f64 {
        self.sample_rate()
    }

    fn quality_indicator(&self) -> PlaybackQuality {
        match self
            .quality_indicator
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            0 => PlaybackQuality::HighQuality,
            1 => PlaybackQuality::OptimizedQuality,
            2 => PlaybackQuality::Underruns,
            3 => PlaybackQuality::Resetting,
            _ => PlaybackQuality::default(),
        }
    }

    fn set_preset(&self, preset: Preset) -> common::error::Result<()> {
        self.node_band_controls.write().retain(|k, _| preset.has(k));
        self.node_key_controls.write().retain(|k, _| preset.has(k));

        self.node_band_controls
            .read()
            .iter()
            .for_each(|(key, shared)| {
                if let Some(val) = preset.get_band_value(key) {
                    shared.set_value(val);
                }
            });
        self.node_key_controls
            .read()
            .iter()
            .for_each(|(key, shared)| {
                if let Some(val) = preset.get_key_value(key) {
                    shared.set_value(val);
                }
            });
        for &key in preset
            .keys()
            .filter(|&k| !self.node_band_controls.read().contains_key(k))
        {
            self.node_band_controls
                .write()
                .insert(key, shared(preset.get_band_value(&key).unwrap()));
        }
        for &key in preset
            .keys()
            .filter(|&k| !self.node_key_controls.read().contains_key(k))
        {
            self.node_key_controls
                .write()
                .insert(key, shared(preset.get_key_value(&key).unwrap()));
        }
        *self.preset.write() = preset;

        Ok(())
    }

    fn get_preset(&self) -> Preset {
        self.preset.read().clone()
    }

    fn snapshot_input_snoop(&self) -> Vec<f32> {
        self.input_snoop
            .write()
            .as_mut()
            .map(|snoop| {
                snoop.update();
                let cap = snoop.capacity();
                let mut out = Vec::with_capacity(cap + 2);
                for rev in (0..cap).rev() {
                    let s = snoop.at(rev);
                    if s.is_normal() || s == 0.0 {
                        out.push(snoop.at(rev));
                    } else {
                        out.push(0.0);
                    }
                }
                out
            })
            .unwrap_or_default()
    }

    #[cfg(feature = "editor")]
    fn get_finetuned_values(&self) -> Result<common::commands::edit::FineTunedValuesPayload> {
        let shared_values = self.fine_tuned_shared_values.read();
        Ok(common::commands::edit::FineTunedValuesPayload {
            siren_alpha: shared_values.siren_alpha.value(),
            group_q: shared_values.group_q.value(),
            group_ls_gain_db: shared_values.group_ls_gain_db.value(),
            filter_morph_follow_s: shared_values.filter_morph_follow_s.value(),
            node_follow_response_time_s: shared_values.node_follow_response_time_s.value(),
            filter_q_piercing: shared_values.filter_q_piercing.value(),
            filter_q_bright: shared_values.filter_q_bright.value(),
            filter_q_shelf: shared_values.filter_q_shelf.value(),
            filter_shelf_gain_db: shared_values.filter_shelf_gain_db.value(),
            filter_q_warm: shared_values.filter_q_warm.value(),
            node_bell_q: shared_values.node_bell_q.value(),
            node_bell_gain_db: shared_values.node_bell_gain_db.value(),
            formant_base_q: shared_values.formant_base_q.value(),
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
            shared_values
                .node_follow_response_time_s
                .set_value(payload.node_follow_response_time_s);
            shared_values.group_q.set_value(payload.group_q);
            shared_values
                .group_ls_gain_db
                .set_value(payload.group_ls_gain_db);
            shared_values
                .filter_morph_follow_s
                .set_value(payload.filter_morph_follow_s);
            shared_values
                .filter_q_piercing
                .set_value(payload.filter_q_piercing);
            shared_values
                .filter_q_bright
                .set_value(payload.filter_q_bright);
            shared_values
                .filter_q_shelf
                .set_value(payload.filter_q_shelf);
            shared_values
                .filter_shelf_gain_db
                .set_value(payload.filter_shelf_gain_db);
            shared_values.filter_q_warm.set_value(payload.filter_q_warm);
            shared_values.node_bell_q.set_value(payload.node_bell_q);
            shared_values
                .node_bell_gain_db
                .set_value(payload.node_bell_gain_db);
            shared_values
                .formant_base_q
                .set_value(payload.formant_base_q);
        }

        self.update_primary_node(&self.last_config.read(), &self.last_tuner_config.read());

        Ok(())
    }
}
/// Factory exposed to the runtime facade.
pub fn make_stream_controller() -> Result<Box<dyn AudioRuntime + Send + Sync>> {
    Ok(Box::new(CpalController::default()))
}
