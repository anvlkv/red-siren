use std::{cmp, ops::Add};

use num_traits::Float;
use parking_lot::RwLock;

use crate::{
    audio_runtime::SampleType,
    dsp::{DspUnit, NetTickData, Siren, SirenConfig, Snapshot},
};

use super::{DspNetwork, DspNetworkBackend};

pub struct Synthesizer {
    pub siren: Siren,
    pub siren_snapshot: Snapshot<2>,
    pub sample_type: RwLock<SampleType>,
    pub sample_rate: RwLock<u32>,
}

impl Synthesizer {
    pub fn new() -> Self {
        // let cfgs = (512..=1024)
        //     .flat_map(SirenConfig::configs_for_resolution)
        //     .collect::<Vec<_>>();
        // let config = cfgs
        //     .into_iter()
        //     .min_by(|a, b| {
        //         a.score()
        //             .partial_cmp(&b.score())
        //             .unwrap_or(cmp::Ordering::Equal)
        //     })
        //     .unwrap();

        // log::info!("Selected SirenConfig: {:#?}", config);

        Synthesizer {
            siren: Siren::new(SirenConfig {
                resolution: 1024,
                n_chambers: 1,
                fib_n_start: 3,
                base_opening_width: 240,
            }),
            siren_snapshot: Snapshot::new(1024),
            sample_type: RwLock::new(SampleType::default()),
            sample_rate: RwLock::new(44100),
        }
    }

    fn make_backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> impl FnMut(&[f32], &mut [f32]) + Send + Sync {
        let mut tick_data = NetTickData {
            time: 0.0,
            sample_rate: *self.sample_rate.read(),
        };
        let siren_snapshot_pipe = self.siren.pipe(&self.siren_snapshot);
        let mut siren_be = siren_snapshot_pipe.backend::<S>();
        let siren_input_frame = Siren::input_frame::<S>();
        let mut siren_output_frame = Siren::output_frame::<S>();

        let sample_duration = 1.0 / tick_data.sample_rate as f64;

        move |_input: &[f32], output: &mut [f32]| {
            siren_be.process(&tick_data, &siren_input_frame, &mut siren_output_frame);

            output
                .iter_mut()
                .zip(siren_output_frame.iter())
                .for_each(|(o, f)| *o = f.to_f32().unwrap());

            tick_data.time = if tick_data.time.add(sample_duration).is_finite() {
                tick_data.time + sample_duration
            } else {
                sample_duration - (f64::MAX - tick_data.time)
            };
        }
    }
}

impl DspNetwork for Synthesizer {
    fn backend(&self) -> Box<dyn DspNetworkBackend + Send + Sync> {
        match *self.sample_type.read() {
            SampleType::F32 => Box::new(self.make_backend::<f32>()),
            SampleType::F64 => Box::new(self.make_backend::<f64>()),
        }
    }

    fn set_sample_rate(&self, sample_rate: u32) {
        // Set the sample rate for the synthesizer
    }

    fn set_sample_type(&self, sample_type: SampleType) {
        // Set the sample type for the synthesizer
    }

    fn fade_in(&self, duration: f64) {
        // Implement fade-in logic for the synthesizer
    }

    fn fade_out(&self, duration: f64) {
        // Implement fade-out logic for the synthesizer
    }
}
