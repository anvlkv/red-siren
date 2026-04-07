use std::{collections::HashMap, f32, sync::Arc, thread, time::Duration};

#[cfg(feature = "editor")]
use common::commands::edit::FineTunedValuesPayload;
use common::error::ControlError;
use common::{
    instrument::{Config as InstrumentConfig, Layout as InstrumentLayout, Preset},
    tuner::Config as TunerConfig,
    NodeKey,
};
use fundsp::{prelude::*, typenum::Unsigned as _};
use parking_lot::RwLock;
use u_num_it::u_num_it;

#[cfg(feature = "editor")]
use crate::system::values::{FineTunedSharedValues, FineTunedValues};
use crate::{
    input::analyzer::SpectrumBuffer, output_analyzer::OUTPUT_ANALYZER_FFT_WINDOW_SIZE,
    quality::SampleType, rt::ExcitementSource, ExcitementControl, SensorHandles, FFT_WINDOW_SIZE,
};

pub const FADE_DURATION_MS: u64 = 120;
const INPUT_SNOOP_SIZE: usize = FFT_WINDOW_SIZE;
const FOLLOW_RESPONSE_SECS: f32 = FADE_DURATION_MS as f32 / 1000.0;

#[derive(Clone)]
pub struct RuntimeSubsystem {
    sample_type: SampleType,
    num_channels: u8,
    sample_rate: Arc<RwLock<f64>>,
    dsp_net: Arc<RwLock<Net>>,
    dsp_primary_node_id: Arc<RwLock<NodeId>>,
    dsp_tuner_node_id: Arc<RwLock<NodeId>>,
    gain_param: Arc<Shared>,

    processed_output_snoops: Arc<RwLock<(Snoop, Snoop)>>,
    input_snoop: Arc<RwLock<Option<Snoop>>>,
    node_excitement_snoops: Arc<RwLock<HashMap<NodeKey, (Snoop, Snoop)>>>,
    node_output_snoops: Arc<RwLock<HashMap<NodeKey, Snoop>>>,

    pub(crate) preset: Arc<RwLock<Preset>>,
    node_band_controls: Arc<RwLock<HashMap<NodeKey, Shared>>>,
    node_key_controls: Arc<RwLock<HashMap<NodeKey, Shared>>>,
    node_sensor_controls: Arc<RwLock<HashMap<NodeKey, SensorHandles>>>,
    siren_excitements: Arc<RwLock<HashMap<NodeKey, ExcitementControl>>>,

    tuner_tap_gain_param: Arc<Shared>,
    tuner_freq_range: Arc<(Shared, Shared)>,
    tuner_ny_threshold: Arc<Shared>,
    tuner_ny_wet_ratio: Arc<Shared>,
    spectrum_data_thb: SpectrumBuffer,

    pub(crate) layout: Arc<RwLock<InstrumentLayout>>,
    pub(crate) config: Arc<RwLock<InstrumentConfig>>,
    pub(crate) source: Arc<RwLock<ExcitementSource>>,
    pub(crate) tuner_config: Arc<RwLock<TunerConfig>>,

    #[cfg(feature = "editor")]
    fine_tuned_shared_values: Arc<RwLock<FineTunedSharedValues>>,
}

struct CreateMainNetworkReturn {
    gain_param: Shared,
    tuner_tap_gain: Shared,
    processed_output_snoop_l: Snoop,
    processed_output_snoop_r: Snoop,
    instrument_node_id: NodeId,
    tuner_node_id: NodeId,
    net: Net,
}

struct CreateInstrumentNetworkReturn {
    excitement_snoops: HashMap<NodeKey, (Snoop, Snoop)>,
    output_snoops: HashMap<NodeKey, Snoop>,
    band_controls: HashMap<NodeKey, Shared>,
    key_controls: HashMap<NodeKey, Shared>,
    siren_excitements: HashMap<NodeKey, ExcitementControl>,
    net: Net,
}

struct CreateTunerNetworkReturn {
    input_snoop: Option<Snoop>,
    handles: HashMap<NodeKey, SensorHandles>,
    net: Net,
}

impl RuntimeSubsystem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        layout: InstrumentLayout,
        config: InstrumentConfig,
        source: ExcitementSource,
        tuner_config: TunerConfig,
        preset: Preset,
        spectrum_data_thb: SpectrumBuffer,
        sample_type: SampleType,
        num_channels: usize,
        sample_rate: f64,
    ) -> Self {
        #[cfg(feature = "editor")]
        let fine_tuned_shared_values = Arc::new(RwLock::new(FineTunedSharedValues::new()));

        #[cfg(feature = "editor")]
        let fine_tuned_values = Self::snapshot_fine_tuned_values(&fine_tuned_shared_values);

        let CreateInstrumentNetworkReturn {
            band_controls,
            key_controls,
            siren_excitements,
            excitement_snoops,
            output_snoops,
            net,
        } = Self::create_instrument_network(
            num_channels,
            sample_rate,
            sample_type,
            &config,
            &preset,
            #[cfg(feature = "editor")]
            &fine_tuned_values,
        );
        let tuner_freq_range = Arc::new((
            shared(tuner_config.frequency_range.0.unwrap_or(f32::NEG_INFINITY)),
            shared(tuner_config.frequency_range.1.unwrap_or(f32::INFINITY)),
        ));
        let tuner_ny_threshold = Arc::new(shared(tuner_config.ny_threshold));
        let tuner_ny_wet_ratio = Arc::new(shared(tuner_config.ny_wet_ratio));
        let CreateTunerNetworkReturn {
            input_snoop,
            handles: sensor_controls,
            net: tuner_net,
        } = Self::create_tuner_network(
            sample_rate,
            sample_type,
            source,
            &tuner_config,
            siren_excitements.clone(),
            &spectrum_data_thb,
            (&tuner_freq_range.0, &tuner_freq_range.1),
            &tuner_ny_threshold,
            &tuner_ny_wet_ratio,
        );

        let CreateMainNetworkReturn {
            gain_param,
            tuner_tap_gain,
            processed_output_snoop_l,
            processed_output_snoop_r,
            instrument_node_id,
            tuner_node_id,
            net: dsp_net,
        } = Self::create_main_network(num_channels, sample_rate, net, tuner_net);

        Self {
            sample_type,
            num_channels: num_channels as u8,
            sample_rate: Arc::new(RwLock::new(sample_rate)),
            dsp_net: Arc::new(RwLock::new(dsp_net)),
            dsp_primary_node_id: Arc::new(RwLock::new(instrument_node_id)),
            dsp_tuner_node_id: Arc::new(RwLock::new(tuner_node_id)),
            gain_param: Arc::new(gain_param),

            processed_output_snoops: Arc::new(RwLock::new((
                processed_output_snoop_l,
                processed_output_snoop_r,
            ))),
            input_snoop: Arc::new(RwLock::new(input_snoop)),
            node_excitement_snoops: Arc::new(RwLock::new(excitement_snoops)),
            node_output_snoops: Arc::new(RwLock::new(output_snoops)),

            preset: Arc::new(RwLock::new(preset)),
            node_band_controls: Arc::new(RwLock::new(band_controls)),
            node_key_controls: Arc::new(RwLock::new(key_controls)),
            node_sensor_controls: Arc::new(RwLock::new(sensor_controls)),
            siren_excitements: Arc::new(RwLock::new(siren_excitements)),

            tuner_tap_gain_param: Arc::new(tuner_tap_gain),
            tuner_freq_range,
            tuner_ny_threshold,
            tuner_ny_wet_ratio,
            spectrum_data_thb,

            layout: Arc::new(RwLock::new(layout)),
            config: Arc::new(RwLock::new(config)),
            source: Arc::new(RwLock::new(source)),
            tuner_config: Arc::new(RwLock::new(tuner_config)),

            #[cfg(feature = "editor")]
            fine_tuned_shared_values,
        }
    }

    pub fn new_with_tuner_only(
        layout: InstrumentLayout,
        tuner_config: TunerConfig,
        spectrum_data_thb: SpectrumBuffer,
        sample_type: SampleType,
        num_channels: usize,
        sample_rate: f64,
    ) -> Self {
        #[cfg(feature = "editor")]
        let fine_tuned_shared_values = Arc::new(RwLock::new(FineTunedSharedValues::new()));

        let CreateInstrumentNetworkReturn {
            band_controls,
            key_controls,
            siren_excitements,
            excitement_snoops,
            output_snoops,
            net,
        } = Self::create_instrument_dummy(num_channels, &layout);
        let source = ExcitementSource::Mic;
        let tuner_freq_range = Arc::new((
            shared(tuner_config.frequency_range.0.unwrap_or(f32::NEG_INFINITY)),
            shared(tuner_config.frequency_range.1.unwrap_or(f32::INFINITY)),
        ));
        let tuner_ny_threshold = Arc::new(shared(tuner_config.ny_threshold));
        let tuner_ny_wet_ratio = Arc::new(shared(tuner_config.ny_wet_ratio));
        let CreateTunerNetworkReturn {
            input_snoop,
            handles: sensor_controls,
            net: tuner_net,
        } = Self::create_tuner_network(
            sample_rate,
            sample_type,
            source,
            &tuner_config,
            siren_excitements.clone(),
            &spectrum_data_thb,
            (&tuner_freq_range.0, &tuner_freq_range.1),
            &tuner_ny_threshold,
            &tuner_ny_wet_ratio,
        );

        let CreateMainNetworkReturn {
            gain_param,
            tuner_tap_gain,
            processed_output_snoop_l,
            processed_output_snoop_r,
            instrument_node_id,
            tuner_node_id,
            net: dsp_net,
        } = Self::create_main_network(num_channels, sample_rate, net, tuner_net);

        Self {
            sample_type,
            num_channels: num_channels as u8,
            sample_rate: Arc::new(RwLock::new(sample_rate)),
            dsp_net: Arc::new(RwLock::new(dsp_net)),
            dsp_primary_node_id: Arc::new(RwLock::new(instrument_node_id)),
            dsp_tuner_node_id: Arc::new(RwLock::new(tuner_node_id)),
            gain_param: Arc::new(gain_param),

            processed_output_snoops: Arc::new(RwLock::new((
                processed_output_snoop_l,
                processed_output_snoop_r,
            ))),
            input_snoop: Arc::new(RwLock::new(input_snoop)),
            node_excitement_snoops: Arc::new(RwLock::new(excitement_snoops)),
            node_output_snoops: Arc::new(RwLock::new(output_snoops)),

            preset: Arc::new(RwLock::new(Default::default())),
            node_band_controls: Arc::new(RwLock::new(band_controls)),
            node_key_controls: Arc::new(RwLock::new(key_controls)),
            node_sensor_controls: Arc::new(RwLock::new(sensor_controls)),
            siren_excitements: Arc::new(RwLock::new(siren_excitements)),

            tuner_tap_gain_param: Arc::new(tuner_tap_gain),
            tuner_freq_range,
            tuner_ny_threshold,
            tuner_ny_wet_ratio,
            spectrum_data_thb,

            layout: Arc::new(RwLock::new(layout)),
            config: Arc::new(RwLock::new(Default::default())),
            source: Arc::new(RwLock::new(source)),
            tuner_config: Arc::new(RwLock::new(tuner_config)),

            #[cfg(feature = "editor")]
            fine_tuned_shared_values,
        }
    }

    #[allow(dead_code)]
    pub fn restart_with_sample_type(&self, sample_type: SampleType) -> Self {
        self.fade_out();
        RuntimeSubsystem::new(
            *self.layout.read(),
            self.config.read().clone(),
            *self.source.read(),
            self.tuner_config.read().clone(),
            self.get_preset(),
            self.spectrum_data_thb.clone(),
            sample_type,
            self.num_channels as usize,
            *self.sample_rate.read(),
        )
    }

    pub fn restart_with_tuner_only(&self, tuner_config: Option<TunerConfig>) -> Self {
        self.fade_out();
        let Self {
            sample_type,
            num_channels,
            sample_rate,
            spectrum_data_thb,
            layout,
            config,
            tuner_config: old_tuner_config,
            ..
        } = self.clone();
        let tuner_only = RuntimeSubsystem::new_with_tuner_only(
            *layout.read(),
            tuner_config.unwrap_or_else(|| old_tuner_config.read().clone()),
            spectrum_data_thb.clone(),
            sample_type,
            num_channels as usize,
            *sample_rate.read(),
        );
        Self {
            layout,
            config,
            ..tuner_only
        }
    }

    pub fn update_tuner_config(&self, tuner_config: &TunerConfig) {
        // 1. Update Nyquist Shared params in the live signal graph (zero DSP rebuild).
        self.tuner_ny_threshold.set_value(tuner_config.ny_threshold);
        self.tuner_ny_wet_ratio.set_value(tuner_config.ny_wet_ratio);

        // 2. Update frequency-range Shared params.
        self.tuner_freq_range
            .0
            .set_value(tuner_config.frequency_range.0.unwrap_or(f32::NEG_INFINITY));
        self.tuner_freq_range
            .1
            .set_value(tuner_config.frequency_range.1.unwrap_or(f32::INFINITY));

        // 3. Push updated sensor bounds into the per-node Shared params.
        {
            let handles = self.node_sensor_controls.read();
            for sensor in &tuner_config.sensor_data {
                if let Some(h) = handles.get(&sensor.key) {
                    h.min_frequency.set_value(sensor.min_frequency);
                    h.max_frequency.set_value(sensor.max_frequency);
                    h.min_magnitude.set_value(sensor.min_magnitude);
                    h.max_magnitude.set_value(sensor.max_magnitude);
                }
            }
        }

        // 4. Store updated config.
        *self.tuner_config.write() = tuner_config.clone();
    }

    pub fn backend(&self) -> NetBackend {
        self.dsp_net.write().backend()
    }

    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        {
            let mut dsp_lock = self.dsp_net.write();
            dsp_lock.set_sample_rate(sample_rate);
            dsp_lock.commit();
        }
        *self.sample_rate.write() = sample_rate;
    }

    pub fn update_preset(&self, new_preset: Preset) {
        let should_update_nets = {
            let mut preset_lock = self.preset.write();
            let update = new_preset.keys().any(|k| !preset_lock.has(k))
                && preset_lock.keys().any(|k| !new_preset.has(k));
            *preset_lock = new_preset.clone();
            update
        };

        if should_update_nets {
            let instrument_net = {
                let config = self.config.read();
                #[cfg(feature = "editor")]
                let fine_tuned_values =
                    Self::snapshot_fine_tuned_values(&self.fine_tuned_shared_values);

                let CreateInstrumentNetworkReturn {
                    band_controls,
                    key_controls,
                    siren_excitements,
                    excitement_snoops,
                    output_snoops,
                    net,
                } = Self::create_instrument_network(
                    self.num_channels as usize,
                    *self.sample_rate.read(),
                    self.sample_type,
                    &config,
                    &new_preset,
                    #[cfg(feature = "editor")]
                    &fine_tuned_values,
                );

                *self.node_band_controls.write() = band_controls;
                *self.node_key_controls.write() = key_controls;
                *self.siren_excitements.write() = siren_excitements;
                *self.node_excitement_snoops.write() = excitement_snoops;
                *self.node_output_snoops.write() = output_snoops;
                net
            };
            self.replace_network(instrument_net, &self.dsp_primary_node_id.read());
        } else {
            self.node_band_controls.write().iter().for_each(|(k, v)| {
                if let Some(val) = new_preset.get_band_value(k) {
                    v.set_value(val);
                }
            });
            self.node_key_controls.write().iter().for_each(|(k, v)| {
                if let Some(val) = new_preset.get_key_value(k) {
                    v.set_value(val);
                }
            });
        }
    }

    pub fn get_preset(&self) -> Preset {
        self.preset.read().clone()
    }

    pub fn set_band_control(&self, key: NodeKey, value: f32) -> common::error::Result<()> {
        self.preset.write().set_band_value(&key, value);
        let controls = self.node_band_controls.read();
        if let Some(control) = controls.get(&key) {
            control.set_value(value);
            Ok(())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    pub fn get_band_control(&self, key: NodeKey) -> common::error::Result<f32> {
        let controls = self.node_band_controls.read();
        if let Some(control) = controls.get(&key) {
            Ok(control.value())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    pub fn set_key_control(&self, key: NodeKey, value: f32) -> common::error::Result<()> {
        self.preset.write().set_key_value(&key, value);
        let controls = self.node_key_controls.read();
        if let Some(control) = controls.get(&key) {
            control.set_value(value);
            Ok(())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    pub fn get_key_control(&self, key: NodeKey) -> common::error::Result<f32> {
        let controls = self.node_key_controls.read();
        if let Some(control) = controls.get(&key) {
            Ok(control.value())
        } else {
            Err(ControlError::NodeNotFound { key }.into())
        }
    }

    // ── Snoop snapshots ──────────────────────────────────────────────────

    /// Read the latest output samples for a single node.
    /// `Snoop::update()` pulls buffered samples from the DSP thread.
    pub fn snapshot_output_snoop(&self, key: NodeKey) -> Vec<f32> {
        let mut snoops = self.node_output_snoops.write();
        let mut out = Vec::new();
        if let Some(snoop) = snoops.get_mut(&key) {
            snoop.update();
            let cap = snoop.capacity();
            out.reserve(cap);
            for rev in (0..cap).rev() {
                let s = snoop.at(rev);
                out.push(if s.is_normal() || s == 0.0 { s } else { 0.0 });
            }
        }
        out
    }

    /// Read the latest output samples for every node, keyed by [`NodeKey`].
    pub fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
        let keys: Vec<NodeKey> = self.node_output_snoops.read().keys().copied().collect();
        keys.into_iter()
            .map(|k| {
                let snap = self.snapshot_output_snoop(k);
                (k, snap)
            })
            .collect()
    }

    /// Read the latest excitement `(primary, secondary)` samples for a single node.
    pub fn snapshot_excitement_snoop(&self, key: NodeKey) -> Vec<(f32, f32)> {
        let mut snoops = self.node_excitement_snoops.write();
        let mut out = Vec::new();
        if let Some((primary, secondary)) = snoops.get_mut(&key) {
            primary.update();
            secondary.update();
            let cap = primary.capacity();
            out.reserve(cap);
            for rev in (0..cap).rev() {
                let p = primary.at(rev);
                let s = secondary.at(rev);
                out.push((
                    if p.is_normal() || p == 0.0 { p } else { 0.0 },
                    if s.is_normal() || s == 0.0 { s } else { 0.0 },
                ));
            }
        }
        out
    }

    /// Read the latest excitement samples for every node.
    pub fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)> {
        let keys: Vec<NodeKey> = self.node_excitement_snoops.read().keys().copied().collect();
        keys.into_iter()
            .map(|k| {
                let snap = self.snapshot_excitement_snoop(k);
                (k, snap)
            })
            .collect()
    }

    /// Read the latest microphone input samples.
    /// Returns an empty `Vec` when the excitement source is not [`ExcitementSource::Mic`].
    pub fn snapshot_input_snoop(&self) -> Vec<f32> {
        let mut guard = self.input_snoop.write();
        guard
            .as_mut()
            .map(|snoop| {
                snoop.update();
                let cap = snoop.capacity();
                let mut out = Vec::with_capacity(cap);
                for rev in (0..cap).rev() {
                    let s = snoop.at(rev);
                    out.push(if s.is_normal() || s == 0.0 { s } else { 0.0 });
                }
                out
            })
            .unwrap_or_default()
    }

    /// Take an FFT snapshot of the processed stereo output.
    ///
    /// Returns `None` when not enough samples have accumulated yet
    /// (needs a full [`OUTPUT_ANALYZER_FFT_WINDOW_SIZE`] frame).
    pub fn snapshot_processed_output_spectrum(
        &self,
    ) -> common::error::Result<Option<super::ProcessedOutputSpectrumSnapshot>> {
        let sample_rate = *self.sample_rate.read();
        let min_hz = self.tuner_freq_range.0.value();
        let max_hz = self.tuner_freq_range.1.value();

        let mut l_window = [0.0_f32; OUTPUT_ANALYZER_FFT_WINDOW_SIZE];
        let mut r_window = [0.0_f32; OUTPUT_ANALYZER_FFT_WINDOW_SIZE];

        let filled = {
            let mut guard = self.processed_output_snoops.write();
            let (l, r) = &mut *guard;
            l.update();
            r.update();
            // at(0) = most recent; at(cap-1) = oldest.
            // Fill oldest-first so the FFT window is time-ordered.
            let cap = std::cmp::Ord::min(l.capacity(), OUTPUT_ANALYZER_FFT_WINDOW_SIZE);
            for i in 0..cap {
                let rev = cap - 1 - i;
                l_window[i] = l.at(rev);
                r_window[i] = r.at(rev);
            }
            cap
        };

        if filled < OUTPUT_ANALYZER_FFT_WINDOW_SIZE {
            return Ok(None);
        }

        let left = crate::output_analyzer::analyze(l_window, sample_rate, min_hz, max_hz)?;
        let right = crate::output_analyzer::analyze(r_window, sample_rate, min_hz, max_hz)?;
        Ok(Some((left, right)))
    }

    // ── Tuner tap & excitements ──────────────────────────────────────────

    /// Route tuner audio into the output mix (tap gain → 1.0).
    pub fn start_tap_tuner_audio(&self) {
        self.tuner_tap_gain_param.set_value(1.0);
    }

    /// Mute the tuner audio tap (tap gain → 0.0).
    pub fn stop_tap_tuner_audio(&self) {
        self.tuner_tap_gain_param.set_value(0.0);
    }

    /// Return the instantaneous primary excitement level for every node.
    /// Results are sorted by [`NodeKey`] for a stable ordering.
    pub fn poll_tuner_excitements(&self) -> Vec<(NodeKey, f32)> {
        let mut data: Vec<(NodeKey, f32)> = self
            .siren_excitements
            .read()
            .iter()
            .map(|(k, v)| (*k, v.primary_value::<f32>()))
            .collect();
        data.sort_by_key(|(k, _)| *k);
        data
    }

    #[cfg(feature = "editor")]
    pub fn get_finetuned_values(&self) -> common::error::Result<FineTunedValuesPayload> {
        let shared = self.fine_tuned_shared_values.read();
        Ok(Self::fine_tuned_values_payload(&*shared))
    }

    #[cfg(feature = "editor")]
    pub fn set_finetuned_values(
        &self,
        payload: FineTunedValuesPayload,
    ) -> common::error::Result<()> {
        {
            let mut shared = self.fine_tuned_shared_values.write();
            shared.siren_alpha.set_value(payload.siren_alpha);
            shared
                .filter_morph_follow_s
                .set_value(payload.filter_morph_follow_s);
            shared
                .node_follow_response_time_s
                .set_value(payload.node_follow_response_time_s);
            shared.group_q.set_value(payload.group_q);
            shared.group_ls_gain_db.set_value(payload.group_ls_gain_db);
            shared
                .filter_q_piercing
                .set_value(payload.filter_q_piercing);
            shared.filter_q_bright.set_value(payload.filter_q_bright);
            shared.filter_q_shelf.set_value(payload.filter_q_shelf);
            shared
                .filter_shelf_gain_db
                .set_value(payload.filter_shelf_gain_db);
            shared.filter_q_warm.set_value(payload.filter_q_warm);
            shared.node_bell_q.set_value(payload.node_bell_q);
            shared
                .node_bell_gain_db
                .set_value(payload.node_bell_gain_db);
            shared.formant_base_q.set_value(payload.formant_base_q);
        }

        let (config, layout, tuner) = {
            let config = self.config.read().clone();
            let layout = self.layout.read().clone();
            let tuner = self.tuner_config.read().clone();
            (config, layout, tuner)
        };
        self.update_configurations(&config, &layout, &tuner);
        Ok(())
    }

    pub fn update_configurations(
        &self,
        instrument_config: &InstrumentConfig,
        instrument_layout: &InstrumentLayout,
        tuner_config: &TunerConfig,
    ) {
        let instrument_net = {
            let mut layout_lock = self.layout.write();
            let mut config_lock = self.config.write();
            let mut tuner_config_lock = self.tuner_config.write();

            *layout_lock = *instrument_layout;
            *config_lock = instrument_config.clone();
            *tuner_config_lock = tuner_config.clone();

            let preset = self.preset.read();
            #[cfg(feature = "editor")]
            let fine_tuned_values =
                Self::snapshot_fine_tuned_values(&self.fine_tuned_shared_values);

            let CreateInstrumentNetworkReturn {
                excitement_snoops,
                output_snoops,
                band_controls,
                key_controls,
                siren_excitements,
                net,
            } = Self::create_instrument_network(
                self.num_channels as usize,
                *self.sample_rate.read(),
                self.sample_type,
                &config_lock,
                &preset,
                #[cfg(feature = "editor")]
                &fine_tuned_values,
            );

            *self.node_band_controls.write() = band_controls;
            *self.node_key_controls.write() = key_controls;
            *self.siren_excitements.write() = siren_excitements;
            *self.node_excitement_snoops.write() = excitement_snoops;
            *self.node_output_snoops.write() = output_snoops;
            net
        };

        self.replace_network(instrument_net, &self.dsp_primary_node_id.read());
    }

    pub fn fade_out(&self) {
        self.gain_param.set_value(0.0);
        thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    pub fn fade_in(&self) {
        self.gain_param.set_value(1.0);
        thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    /// Replace only the tuner sub-network (1-in/1-out node) without rebuilding the
    /// instrument network or restarting the CPAL stream.
    ///
    /// Handles fade/commit via [`Self::replace_network`] so the audio transition is
    /// seamless when the stream is running.
    pub fn replace_tuner_for_source(&self, source: ExcitementSource) {
        let siren_excitements = self.siren_excitements.read().clone();
        let tuner_config = self.tuner_config.read().clone();
        let sample_rate = *self.sample_rate.read();

        let CreateTunerNetworkReturn {
            input_snoop,
            handles,
            net,
        } = Self::create_tuner_network(
            sample_rate,
            self.sample_type,
            source,
            &tuner_config,
            siren_excitements,
            &self.spectrum_data_thb,
            (&self.tuner_freq_range.0, &self.tuner_freq_range.1),
            &self.tuner_ny_threshold,
            &self.tuner_ny_wet_ratio,
        );

        *self.input_snoop.write() = input_snoop;
        *self.node_sensor_controls.write() = handles;
        *self.source.write() = source;

        self.replace_network(net, &self.dsp_tuner_node_id.read());
    }

    fn replace_network(&self, unit: Net, id: &NodeId) {
        let mut dsp_lock = self.dsp_net.write();
        if dsp_lock.has_backend() {
            // Only fade and commit when the backend (audio stream) is live.
            // Fading without a backend is a no-op but costs 2 × FADE_DURATION_MS.
            drop(dsp_lock);
            self.fade_out();
            {
                let mut dsp_lock = self.dsp_net.write();
                dsp_lock.replace(*id, Box::new(unit));
                dsp_lock.commit();
            }
            self.fade_in();
        } else {
            // Backend not yet attached (stream not started or already stopped).
            // Update the network locally so it is correct the next time the
            // stream starts and calls `sys.backend()`.
            log::debug!("replace_network: no backend attached – updating net node without commit");
            dsp_lock.replace(*id, Box::new(unit));
            // No commit; the next start_output_stream will build a fresh
            // RuntimeSubsystem from the stored layout/config anyway.
        }
    }

    #[cfg(feature = "editor")]
    fn snapshot_fine_tuned_values(shared: &Arc<RwLock<FineTunedSharedValues>>) -> FineTunedValues {
        let guard = shared.read();
        FineTunedValues::new(&*guard)
    }

    #[cfg(feature = "editor")]
    fn fine_tuned_values_payload(shared: &FineTunedSharedValues) -> FineTunedValuesPayload {
        FineTunedValuesPayload {
            siren_alpha: shared.siren_alpha.value(),
            group_q: shared.group_q.value(),
            group_ls_gain_db: shared.group_ls_gain_db.value(),
            filter_morph_follow_s: shared.filter_morph_follow_s.value(),
            node_follow_response_time_s: shared.node_follow_response_time_s.value(),
            filter_q_piercing: shared.filter_q_piercing.value(),
            filter_q_bright: shared.filter_q_bright.value(),
            filter_q_shelf: shared.filter_q_shelf.value(),
            filter_shelf_gain_db: shared.filter_shelf_gain_db.value(),
            filter_q_warm: shared.filter_q_warm.value(),
            node_bell_q: shared.node_bell_q.value(),
            node_bell_gain_db: shared.node_bell_gain_db.value(),
            formant_base_q: shared.formant_base_q.value(),
        }
    }

    fn create_instrument_dummy(
        num_channels: usize,
        layout: &InstrumentLayout,
    ) -> CreateInstrumentNetworkReturn {
        let mut net = Net::new(0, num_channels);
        for _ in 0..num_channels {
            let source = net.push(Box::new(zero()));
            net.pipe_output(source);
        }

        let siren_excitements = HashMap::<NodeKey, ExcitementControl>::from_iter(
            layout
                .registry()
                .all_keys()
                .into_iter()
                .map(|key| (key, ExcitementControl::new(shared(0.0), shared(0.0)))),
        );

        CreateInstrumentNetworkReturn {
            excitement_snoops: HashMap::new(),
            output_snoops: HashMap::new(),
            band_controls: HashMap::new(),
            key_controls: HashMap::new(),
            siren_excitements,
            net,
        }
    }

    fn create_instrument_network(
        num_channels: usize,
        sample_rate: f64,
        sample_type: SampleType,
        config: &InstrumentConfig,
        preset: &Preset,
        #[cfg(feature = "editor")] fine_tuned_values: &FineTunedValues,
    ) -> CreateInstrumentNetworkReturn {
        log::info!(
            "Creating network with {} groups, {} keys per group",
            config.num_groups(),
            config.0.first().map(|g| g.nodes.len()).unwrap_or(0)
        );

        let mut net = Net::new(0, num_channels);

        // Build output system graph & retrieve handles.
        let node_handles = match sample_type {
            SampleType::F32 => crate::mount_output_system::<f32>(
                config,
                &mut net,
                num_channels,
                #[cfg(feature = "editor")]
                &fine_tuned_values,
            ),
            SampleType::F64 => crate::mount_output_system::<f64>(
                config,
                &mut net,
                num_channels,
                #[cfg(feature = "editor")]
                &fine_tuned_values,
            ),
        };

        let mut siren_excitements = HashMap::<NodeKey, ExcitementControl>::new();

        let mut excitement_snoops = HashMap::new();
        let mut output_snoops = HashMap::new();
        let mut band_controls = HashMap::new();
        let mut key_controls = HashMap::new();

        for handle in node_handles {
            excitement_snoops.insert(
                handle.key,
                (handle.excitement_snoop, handle.secondary_excitement_snoop),
            );
            output_snoops.insert(handle.key, handle.output_snoop);
            siren_excitements.insert(handle.key, handle.siren_control);

            if let Some(val) = preset.get_band_value(&handle.key) {
                handle.band_control.set_value(val);
            }
            if let Some(val) = preset.get_key_value(&handle.key) {
                handle.key_control.set_value(val);
            }

            band_controls.insert(handle.key, handle.band_control);
            key_controls.insert(handle.key, handle.key_control);
        }

        net.set_sample_rate(sample_rate);
        net.allocate();
        net.check();

        log::debug!("created network: {}", net.display());

        CreateInstrumentNetworkReturn {
            excitement_snoops,
            output_snoops,
            band_controls,
            key_controls,
            siren_excitements,
            net,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_tuner_network(
        sample_rate: f64,
        sample_type: SampleType,
        source: ExcitementSource,
        tuner_config: &TunerConfig,
        siren_excitements: HashMap<NodeKey, ExcitementControl>,
        spectrum_data_thb: &SpectrumBuffer,
        tuner_freq_range: (&Shared, &Shared),
        tuner_ny_threshold: &Shared,
        tuner_ny_wet_ratio: &Shared,
    ) -> CreateTunerNetworkReturn {
        log::info!("Creating tuner network");

        let mut net = Net::new(1, 1);

        let (snoop_be, input_snoop) = match source {
            ExcitementSource::Entropy => (None, None),
            ExcitementSource::Mic => {
                let (snoop, be) = snoop(INPUT_SNOOP_SIZE);
                (Some(be), Some(snoop))
            }
        };

        let handles = match sample_type {
            SampleType::F32 => crate::create_input_system::<f32>(
                tuner_config,
                &mut net,
                siren_excitements,
                source,
                spectrum_data_thb,
                snoop_be,
                tuner_freq_range,
                (tuner_ny_threshold, tuner_ny_wet_ratio),
                0,
            ),
            SampleType::F64 => crate::create_input_system::<f64>(
                tuner_config,
                &mut net,
                siren_excitements,
                source,
                spectrum_data_thb,
                snoop_be,
                tuner_freq_range,
                (tuner_ny_threshold, tuner_ny_wet_ratio),
                0,
            ),
        };

        let handles = HashMap::from_iter(handles.into_iter().map(|h| (h.key, h)));

        net.set_sample_rate(sample_rate);
        net.allocate();
        net.check();

        CreateTunerNetworkReturn {
            input_snoop,
            handles,
            net,
        }
    }

    fn create_main_network(
        num_channels: usize,
        sample_rate: f64,
        instrument_subnet: Net,
        tuner_subnet: Net,
    ) -> CreateMainNetworkReturn {
        denormal::prevent_denormals();

        let mut net = Net::new(1, num_channels);

        let instrument_node_id = net.push(Box::new(instrument_subnet));

        let (processed_output_snoop_l, processed_output_snoop_backend_l) =
            snoop(OUTPUT_ANALYZER_FFT_WINDOW_SIZE);
        let (processed_output_snoop_r, processed_output_snoop_backend_r) =
            snoop(OUTPUT_ANALYZER_FFT_WINDOW_SIZE);
        // Insert smoothed gain after main node for fade in/out.
        let gain_param = shared(1.0f32);
        let tuner_tap_gain = shared(0.0f32);
        let processed_output_snoops_id = u_num_it!(
            1..=7,
            match num_channels {
                1 => {
                    net.push(Box::new(
                        split::<U2>()
                            >> (processed_output_snoop_backend_l
                                | processed_output_snoop_backend_r)
                            >> join::<U2>(),
                    ))
                }
                2 => {
                    net.push(Box::new(
                        processed_output_snoop_backend_l | processed_output_snoop_backend_r,
                    ))
                }
                U => {
                    net.push(Box::new(
                        multisplit::<NumType, U2>()
                            >> (multipass::<NumType>()
                                | An(Map::new(
                                    |frame: &Frame<f32, NumType>| -> Frame<f32, U2> {
                                        let mut join_frame = frame.as_slice().chunks(2).fold(
                                            Frame::<f32, U2>::splat(0.0),
                                            |mut acc, frame| {
                                                let f1 = frame.first().unwrap_or(&0.0);
                                                let f2 = frame.get(1).unwrap_or(f1);
                                                acc[0] += f1;
                                                acc[1] += f2;
                                                acc
                                            },
                                        );

                                        join_frame[0] /= (NumType::USIZE as f32) / 2.0;
                                        join_frame[1] /= (NumType::USIZE as f32) / 2.0;
                                        join_frame
                                    },
                                    Routing::Join,
                                )))
                            >> (multipass::<NumType>()
                                | ((processed_output_snoop_backend_l
                                    | processed_output_snoop_backend_r)
                                    >> multisink::<U2>())),
                    ))
                }
                _ => {
                    panic!("Unexpected number of channels: {num_channels}. Supported 1..=7");
                }
            }
        );

        let tuner_node_id = net.push(Box::new(
            tuner_subnet >> (var(&tuner_tap_gain) * delay(0.25)),
        ));

        let gain_id = u_num_it!(
            1..=7,
            match num_channels {
                U => {
                    net.push(Box::new(
                        (multipass::<NumType>() | split::<NumType>())
                            >> multijoin::<NumType, U2>()
                            >> mul(Frame::<f32, NumType>::splat(2.0))
                            >> ((var(&gain_param)
                                >> follow(FOLLOW_RESPONSE_SECS)
                                >> split::<NumType>())
                                * multipass::<NumType>()),
                    ))
                }
                _ => {
                    panic!("Unexpected number of channels: {num_channels}. Supported 1..=7");
                }
            }
        );

        net.pipe_all(instrument_node_id, processed_output_snoops_id);
        net.pipe_all(processed_output_snoops_id, gain_id);
        // Connect the tuner's single output to the last input of gain_id (index = num_channels).
        // We must NOT use pipe_all here: pipe_all iterates over ALL target inputs starting from 0
        // and cycles the source outputs via `channel % source_outputs`, which would overwrite the
        // num_channels instrument inputs that were just wired by the previous pipe_all call.
        net.connect(tuner_node_id, 0, gain_id, num_channels);
        net.pipe_output(gain_id);
        net.pipe_input(tuner_node_id);

        net.set_sample_rate(sample_rate);
        net.allocate();
        net.check();

        CreateMainNetworkReturn {
            gain_param,
            tuner_tap_gain,
            processed_output_snoop_l,
            processed_output_snoop_r,
            instrument_node_id,
            tuner_node_id,
            net,
        }
    }
}
