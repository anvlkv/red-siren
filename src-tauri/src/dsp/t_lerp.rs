use std::sync::{atomic::Ordering, Arc};

use atomic_float::AtomicF64;
use num_traits::Float;

use crate::dsp::{DspUnit, NetTickData};

pub struct TLerpGain {
    pub target_gain: Arc<AtomicF64>,
    pub target_duration: Arc<AtomicF64>,
}

impl TLerpGain {
    pub fn new() -> Self {
        Self {
            target_gain: Arc::new(AtomicF64::new(0_f64)),
            target_duration: Arc::new(AtomicF64::new(0_f64)),
        }
    }
}

impl DspUnit for TLerpGain {
    fn backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn super::DspUnitBackend<S> + Send + Sync> {
        let target_gain = self.target_gain.clone();
        let target_duration = self.target_duration.clone();

        let mut prev_gain = S::zero();

        Box::new(
            move |&NetTickData { delta_time, .. }: &NetTickData, input: &[S], output: &mut [S]| {
                let target_gain = S::from(target_gain.load(Ordering::Relaxed)).unwrap_or(S::zero());
                let target_duration =
                    S::from(target_duration.load(Ordering::Relaxed)).unwrap_or(S::zero());
                let delta_time = S::from(delta_time).unwrap_or(S::zero());

                let delta_gain = target_gain - prev_gain;

                let t_s = if target_duration.is_zero() {
                    S::one()
                } else {
                    target_duration
                };

                let alpha = (S::one() - (-(delta_time / t_s)).exp()).min(S::one());

                let new_gain = prev_gain + alpha * delta_gain;

                output[0] = input[0] * new_gain;

                prev_gain = new_gain;
            },
        )
    }

    fn output_frame<S: num_traits::Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 1]
    }

    fn input_frame<S: num_traits::Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 1]
    }
}
