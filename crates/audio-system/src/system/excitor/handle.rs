use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicI32, AtomicU32},
        Arc,
    },
};

use common::tuner::Config as TunerConfig;
use common::NodeKey;
use common::{instrument::Config as InstrumentConfig, tuner::SensorData};
use fundsp::{prelude::*, thingbuf::ThingBuf};
use spectrum_analyzer::FrequencyLimit;

use crate::excitor::{control::Control, SpectrumBuffer};

pub struct SensorHandle {
    pub min_freq: Shared,
    pub max_freq: Shared,
    pub min_mag: Shared,
    pub max_mag: Shared,
}

#[derive(Clone)]
pub struct AnalyzerHandle {
    pub frequency_limit: Arc<(AtomicI32, AtomicI32)>,
    pub sensors: Arc<BTreeMap<NodeKey, SensorHandle>>,
    pub fft_size: Arc<AtomicU32>,
    pub spectrum_buffer: SpectrumBuffer,
    pub sample_rate: Arc<AtomicU32>,
}

impl AnalyzerHandle {
    pub fn update_frequency_limit(&self, min_frequency: Option<f32>, max_frequency: Option<f32>) {
        self.frequency_limit.0.store(
            min_frequency.map_or(-1, |l| l.round() as i32),
            std::sync::atomic::Ordering::SeqCst,
        );
        self.frequency_limit.1.store(
            max_frequency.map_or(-1, |l| l.round() as i32),
            std::sync::atomic::Ordering::SeqCst,
        );
    }

    pub fn frequency_limit(&self) -> FrequencyLimit {
        match (
            self.frequency_limit
                .0
                .load(std::sync::atomic::Ordering::SeqCst),
            self.frequency_limit
                .1
                .load(std::sync::atomic::Ordering::SeqCst),
        ) {
            (-1, -1) => FrequencyLimit::All,
            (min, -1) => FrequencyLimit::Min(min as f32),
            (-1, max) => FrequencyLimit::Max(max as f32),
            (min, max) => FrequencyLimit::Range(min as f32, max as f32),
        }
    }

    pub fn sensor_data_iter(&self) -> impl Iterator<Item = SensorData> {
        self.sensors.iter().map(|(&key, v)| SensorData {
            key,
            min_frequency: v.min_freq.value(),
            max_frequency: v.max_freq.value(),
            min_magnitude: v.min_mag.value(),
            max_magnitude: v.max_mag.value(),
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Clone)]
pub struct ExcitorHandle {
    pub analyzer: AnalyzerHandle,
    pub controls: Arc<BTreeMap<NodeKey, Control>>,
}

impl ExcitorHandle {
    pub fn new(tuner_config: &TunerConfig, instrument_config: &InstrumentConfig) -> Self {
        let tuner_constraints = tuner_config.constraints();

        let frequency_limit = Arc::new((
            AtomicI32::new(
                tuner_constraints
                    .min_frequency
                    .map_or(-1, |l| l.round() as i32),
            ),
            AtomicI32::new(
                tuner_constraints
                    .max_frequency
                    .map_or(-1, |l| l.round() as i32),
            ),
        ));

        let sensors = Arc::new(BTreeMap::from_iter(tuner_config.sensor_data.iter().map(
            |sd| {
                (
                    sd.key,
                    SensorHandle {
                        min_freq: shared(sd.min_frequency),
                        max_freq: shared(sd.max_frequency),
                        min_mag: shared(sd.min_magnitude),
                        max_mag: shared(sd.max_magnitude),
                    },
                )
            },
        )));

        let fft_size = Arc::new(AtomicU32::new(tuner_config.fft_size as u32));

        let spectrum_buffer = SpectrumBuffer::new(ThingBuf::new(MAX_BUFFER_SIZE));

        let sample_rate = Arc::new(AtomicU32::new(tuner_config.sample_rate.round() as u32));

        let analyzer = AnalyzerHandle {
            frequency_limit,
            sensors,
            fft_size,
            spectrum_buffer,
            sample_rate,
        };

        let controls = Arc::new(
            instrument_config
                .nodes_iter()
                .map(|node| {
                    (
                        node.key,
                        Control {
                            key: node.key,
                            real: shared(0.0),
                            imaginary: shared(0.0),
                        },
                    )
                })
                .collect(),
        );

        ExcitorHandle { analyzer, controls }
    }
}
