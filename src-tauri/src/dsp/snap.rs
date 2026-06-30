use std::{
    array,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use atomic_float::AtomicF32;
use num_traits::Float;

use crate::dsp::DspUnit;

pub struct Snapshot<const N: usize> {
    output: Arc<Vec<[AtomicF32; N]>>,
    pos: Arc<AtomicUsize>,
}

impl<const N: usize> Snapshot<N> {
    pub fn new(resolution: usize) -> Self {
        let output = Arc::new(
            (0..resolution)
                .map(|_| array::from_fn(|_| AtomicF32::new(0.0)))
                .collect::<Vec<_>>(),
        );

        let pos = Arc::new(AtomicUsize::new(0));

        Self { output, pos }
    }

    pub fn get_snapshot(&self) -> Vec<[f32; N]> {
        self.output
            .iter()
            .map(|slot| array::from_fn(|i| slot[i].load(Ordering::Relaxed)))
            .collect()
    }
}

impl<const N: usize> DspUnit for Snapshot<N> {
    fn backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn super::DspUnitBackend<S> + Send + Sync> {
        let output_data = self.output.clone();
        let pos = self.pos.clone();
        Box::new(
            move |_tick_data: &super::NetTickData, input: &[S], output: &mut [S]| {
                debug_assert_eq!(input.len(), N, "Snapshot input length must be equal to N");
                debug_assert_eq!(output.len(), N, "Snapshot output length must be equal to N");

                let pos = {
                    let inner = pos.load(Ordering::Relaxed);
                    if inner >= output_data.len() {
                        pos.store(0, Ordering::Relaxed);
                        0
                    } else {
                        pos.store(inner + 1, Ordering::Relaxed);
                        inner
                    }
                };
                let output_slot = &output_data[pos];
                for (i, slot) in output_slot.iter().enumerate() {
                    // Pass the input value to the output
                    output[i] = input[i];

                    // Store the input value in the snapshot slot
                    slot.store(input[i].to_f32().unwrap_or(0.0), Ordering::Relaxed);
                }
            },
        )
    }

    fn input_frame<S: num_traits::Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); N]
    }

    fn output_frame<S: num_traits::Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); N]
    }
}
