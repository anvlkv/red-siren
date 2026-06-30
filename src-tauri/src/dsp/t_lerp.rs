use std::sync::Arc;

use atomic_float::AtomicF64;

use crate::dsp::DspUnit;

pub struct TLerp {
    pub target_value: Arc<AtomicF64>,
    pub target_t_s: Arc<AtomicF64>,
}

impl DspUnit for TLerp {
    fn backend<S: num_traits::Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn super::DspUnitBackend<S> + Send + Sync> {
        todo!()
    }

    fn output_frame<S: num_traits::Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 1]
    }

    fn input_frame<S: num_traits::Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 1]
    }
}
