use std::{ops::Add, sync::atomic::Ordering};

use num_traits::Float;
use parking_lot::RwLock;

use crate::{
    audio_runtime::SampleType,
    dsp::{t_lerp::TLerpGain, DspUnit, NetTickData, Siren, SirenConfig, Snapshot},
};

use super::{DspNetwork, DspNetworkBackend};

pub struct Synthesizer {
    pub siren: Siren,
    pub t_lerp_gain: TLerpGain,
    pub siren_snapshot: Snapshot<2>,
    pub sample_type: RwLock<SampleType>,
    pub sample_rate: RwLock<u32>,
}

impl Synthesizer {
    pub fn new() -> Self {
        let config = SirenConfig::from_resolution(64).unwrap();

        log::info!("Selected SirenConfig: {:#?}", config);

        Synthesizer {
            siren: Siren::new(config),
            t_lerp_gain: TLerpGain::new(),
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
            delta_time: 0.0,
            sample_rate: *self.sample_rate.read(),
        };
        let siren_snapshot_pipe = self.siren.pipe(&self.siren_snapshot);
        let mut siren_be = siren_snapshot_pipe.backend::<S>();
        let siren_input_frame = Siren::input_frame::<S>();
        let mut siren_output_frame = Siren::output_frame::<S>();
        let mut t_lerp = siren_output_frame
            .iter()
            .map(|_| self.t_lerp_gain.backend::<S>())
            .collect::<Vec<_>>();

        let sample_duration = 1.0 / tick_data.sample_rate as f64;

        move |_input: &[f32], output: &mut [f32]| {
            siren_be.process(&tick_data, &siren_input_frame, &mut siren_output_frame);

            let lerp_output_frame =
                siren_output_frame
                    .iter()
                    .zip(t_lerp.iter_mut())
                    .map(|(&s, t)| {
                        let mut output = [S::zero(); 1];
                        t.process(&tick_data, &[s], &mut output);
                        output[0]
                    });

            output
                .iter_mut()
                .zip(lerp_output_frame)
                .for_each(|(o, f)| *o = f.to_f32().unwrap());

            tick_data.time = if tick_data.time.add(sample_duration).is_finite() {
                tick_data.time + sample_duration
            } else {
                sample_duration - (f64::MAX - tick_data.time)
            };
            tick_data.delta_time = sample_duration;
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
        *self.sample_rate.write() = sample_rate;
    }

    fn set_sample_type(&self, sample_type: SampleType) {
        self.sample_type.write().clone_from(&sample_type);
    }

    fn fade_in(&self, duration: f64) {
        self.t_lerp_gain.target_gain.store(1.0, Ordering::Relaxed);
        self.t_lerp_gain
            .target_duration
            .store(duration, Ordering::Relaxed);
    }

    fn fade_out(&self, duration: f64) {
        self.t_lerp_gain.target_gain.store(0.0, Ordering::Relaxed);
        self.t_lerp_gain
            .target_duration
            .store(duration, Ordering::Relaxed);
    }
}
