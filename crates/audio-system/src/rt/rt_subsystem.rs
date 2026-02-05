use std::{collections::HashMap, f32, sync::Arc, thread, time::Duration};

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
    fine_tuned_shared_values: RwLock<FineTunedSharedValues>,
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
            fine_tuned_shared_values: RwLock::new(FineTunedSharedValues::new()),
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
            fine_tuned_shared_values: RwLock::new(FineTunedSharedValues::new()),
        }
    }

    pub fn restart_with_sample_type(&self, sample_type: SampleType) -> Self {
        self.fade_out();
        RuntimeSubsystem::new(
            self.layout.read().clone(),
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
            layout.read().clone(),
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
        let mut tuner_config_lock = self.tuner_config.write();
        *tuner_config_lock = tuner_config.clone();
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
                let fine_tuned_values = self.fine_tuned_shared_values.read();

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
                v.set_value(new_preset.get_band_value(k).unwrap());
            });
            self.node_key_controls.write().iter().for_each(|(k, v)| {
                v.set_value(new_preset.get_key_value(k).unwrap());
            });
        }
    }

    pub fn get_preset(&self) -> Preset {
        self.preset.read().clone()
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

            *layout_lock = instrument_layout.clone();
            *config_lock = instrument_config.clone();
            *tuner_config_lock = tuner_config.clone();

            let preset = self.preset.read();
            #[cfg(feature = "editor")]
            let fine_tuned_values = self.fine_tuned_shared_values.read();

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

    fn replace_network(&self, unit: Net, id: &NodeId) {
        self.fade_out();
        {
            let mut dsp_lock = self.dsp_net.write();
            dsp_lock.replace(*id, Box::new(unit));
            dsp_lock.commit();
        }
        self.fade_in();
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
            SampleType::F32 => crate::create_output_system::<f32>(
                config,
                &mut net,
                num_channels,
                #[cfg(feature = "editor")]
                &fine_tuned_values,
            ),
            SampleType::F64 => crate::create_output_system::<f64>(
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
                                                let f1 = frame.get(0).unwrap_or(&0.0);
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
        net.pipe_all(tuner_node_id, gain_id);
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
