use std::sync::Arc;

use atomic_float::AtomicF32;
use num_traits::Float;

use crate::dsp::{DspUnit, DspUnitBackend, NetTickData};

#[derive(Clone)]
pub struct EnergySource {
    pub frequency: Arc<AtomicF32>,
}

impl EnergySource {
    pub fn new(initial_frequency: f32) -> Self {
        Self {
            frequency: Arc::new(AtomicF32::new(initial_frequency)),
        }
    }
}

impl DspUnit for EnergySource {
    fn backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn DspUnitBackend<S> + Send + Sync> {
        let frequency = self.frequency.clone();
        let mut phase = S::zero();

        Box::new(
            move |&NetTickData { sample_rate, .. }: &NetTickData,
                  _input: &[S],
                  output: &mut [S]| {
                output[0] = S::from(1).unwrap();
                // let sample_rate = S::from(sample_rate).unwrap_or(S::one());
                // let frequency = S::from(frequency.load(std::sync::atomic::Ordering::Relaxed))
                //     .unwrap_or(S::zero());
                // let increment = S::from(2.0 * std::f32::consts::PI).unwrap_or(S::zero())
                //     * frequency
                //     / sample_rate;

                // output[0] = phase.sin();

                // phase = increment + phase;
                // if phase > S::from(2.0 * std::f32::consts::PI).unwrap_or(S::zero()) {
                //     phase = phase - S::from(2.0 * std::f32::consts::PI).unwrap_or(S::zero());
                // }
            },
        )
    }

    fn output_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 1]
    }

    fn input_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![]
    }
}
