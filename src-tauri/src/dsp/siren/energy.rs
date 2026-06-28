use std::sync::Arc;

use atomic_float::AtomicF32;
use num_traits::Float;

use crate::dsp::{DspUnit, DspUnitBackend, NetTickData};

#[derive(Clone)]
pub struct EnergySource {
    pub frequency: Arc<AtomicF32>,
    pub amplitude_gain: Arc<AtomicF32>,
}

impl EnergySource {
    pub fn new(initial_frequency: f32, initial_amplitude_gain: f32) -> Self {
        Self {
            frequency: Arc::new(AtomicF32::new(initial_frequency)),
            amplitude_gain: Arc::new(AtomicF32::new(initial_amplitude_gain)),
        }
    }
}

impl DspUnit for EnergySource {
    fn backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn DspUnitBackend<S> + Send + Sync> {
        let frequency = self.frequency.clone();
        let amplitude_gain = self.amplitude_gain.clone();
        let mut phase = S::zero();

        Box::new(
            move |&NetTickData { sample_rate, .. }: &NetTickData,
                  _input: &[S],
                  output: &mut [S]| {
                let sample_rate = S::from(sample_rate).unwrap_or(S::one());
                let frequency = S::from(frequency.load(std::sync::atomic::Ordering::Relaxed))
                    .unwrap_or(S::zero());
                let increment = S::from(2.0 * std::f32::consts::PI).unwrap_or(S::zero())
                    * frequency
                    / sample_rate;
                let amplitude_gain =
                    S::from(amplitude_gain.load(std::sync::atomic::Ordering::Relaxed))
                        .unwrap_or(S::one());
                output[0] = phase.sin() * amplitude_gain;

                phase = increment + phase;
                if phase > S::from(2.0 * std::f32::consts::PI).unwrap_or(S::zero()) {
                    phase = phase - S::from(2.0 * std::f32::consts::PI).unwrap_or(S::zero());
                }
            },
        )
    }

    fn output_buffer<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 1]
    }

    fn input_buffer<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![]
    }
}
