use std::sync::{atomic::Ordering, Arc};

use atomic_float::AtomicF64;

use crate::dsp::{DspUnit, NetTickData};

/// Speed of sound in air at 20 degrees Celsius in meters per second
const C_MS: f64 = 343.0;
const LOG_1000: f64 = 6.907755278982137;

/// Helmholtz resonator model based on the following equations:
///
/// f_resonance = (c / tau) * sqrt(neck_area / (cavity_volume * eff_neck_length))
/// omega = tau * f_resonance / sample_rate
/// resonance = exp(-log(1000) / (t60 * sample_rate))
///
/// y[n] = x[n] + a1 * y[n-1] + a2 * y[n-2]
#[derive(Clone)]
pub struct ResonatorHh {
    pub cavity_volume: Arc<AtomicF64>,
    pub cs_neck_radius: Arc<AtomicF64>,
    pub eff_neck_length: Arc<AtomicF64>,
    pub t60: Arc<AtomicF64>,
}

impl ResonatorHh {
    pub fn new() -> Self {
        Self {
            cavity_volume: Arc::new(AtomicF64::new(0.001)),
            cs_neck_radius: Arc::new(AtomicF64::new(0.01)),
            eff_neck_length: Arc::new(AtomicF64::new(0.01)),
            t60: Arc::new(AtomicF64::new(1.0)),
        }
    }
}

impl DspUnit for ResonatorHh {
    fn backend<S: num_traits::Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn super::DspUnitBackend<S> + Send + Sync> {
        let Self {
            cavity_volume,
            cs_neck_radius,
            eff_neck_length,
            t60,
        } = self.clone();

        let tau = S::from(std::f64::consts::TAU).unwrap();
        let c = S::from(C_MS).unwrap();
        let log_1000 = S::from(LOG_1000).unwrap();
        let mut prev_output = [S::zero(); 2];

        Box::new(
            move |&NetTickData { sample_rate, .. }: &NetTickData, input: &[S], output: &mut [S]| {
                let cavity_volume = S::from(cavity_volume.load(Ordering::Relaxed)).unwrap();
                let cs_neck_radius = S::from(cs_neck_radius.load(Ordering::Relaxed)).unwrap();
                let eff_neck_length = S::from(eff_neck_length.load(Ordering::Relaxed)).unwrap();
                let t60 = S::from(t60.load(Ordering::Relaxed)).unwrap();
                let sample_rate = S::from(sample_rate).unwrap();

                let neck_area = S::from(std::f64::consts::PI).unwrap() * cs_neck_radius.powi(2);

                let f_resonance =
                    (c / tau) * (neck_area / (cavity_volume * eff_neck_length)).sqrt();
                let omega = tau * f_resonance / sample_rate;
                let resonance = (-log_1000 / (t60 * sample_rate)).exp();

                let a1 = S::from(2).unwrap() * resonance * omega.cos();
                let a2 = -resonance * resonance;

                let y = input[0] + a1 * prev_output[0] + a2 * prev_output[1];
                output[0] = y;
                prev_output[1] = prev_output[0];
                prev_output[0] = y;
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
