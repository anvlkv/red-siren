use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use atomic_float::AtomicF64;
use num_traits::Float;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dsp::{resonator::ResonatorHh, DspUnit, DspUnitBackend, NetTickData};

pub struct Chamber {
    pub is_left_channel: bool,
    pub wheel: Arc<Vec<AtomicF64>>,
    pub window_size: Arc<AtomicU32>,
    pub speed: Arc<AtomicU32>,
    pub resonators: Arc<[ResonatorHh; 4]>,
    pub opening_width: usize,
    pub gap_width: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChamberInfo {
    pub resolution: u32,
    pub is_left_channel: bool,
    pub wheel: Vec<f64>,
    pub window_size: u32,
}

pub struct ShaperCallbackArgs {
    pub chunk_index: usize,
    pub num_chunks: usize,
    pub index: usize,
    pub opening_width: usize,
    pub gap_width: usize,
    pub previous_chord: f64,
}

impl Chamber {
    pub fn new(
        resolution: usize,
        is_left_channel: bool,
        initial_window_size: u32,
        opening_width: usize,
        gap_width: usize,
    ) -> Self {
        let wheel = Arc::new(
            (0..resolution)
                .map(|_| AtomicF64::new(0_f64))
                .collect::<Vec<_>>(),
        );
        let window_size = Arc::new(AtomicU32::new(initial_window_size));
        let speed = Arc::new(AtomicU32::new(0));
        let resonators = Arc::new([
            ResonatorHh::new(),
            ResonatorHh::new(),
            ResonatorHh::new(),
            ResonatorHh::new(),
        ]);

        Self {
            is_left_channel,
            wheel,
            window_size,
            speed,
            resonators,
            opening_width,
            gap_width,
        }
    }

    pub fn shape<F>(&self, mut shaper: F) -> ChamberInfo
    where
        F: FnMut(&ShaperCallbackArgs) -> f64,
    {
        let num_chunks = self.wheel.len() / (self.opening_width + self.gap_width);
        let slice_start = self.gap_width / 2;
        let slice_end = slice_start + self.opening_width;

        for (chunk_index, chunk) in self
            .wheel
            .chunks(self.opening_width + self.gap_width)
            .enumerate()
        {
            if chunk.len() < slice_end {
                continue;
            }
            chunk[slice_start..slice_end]
                .iter()
                .enumerate()
                .for_each(|(index, chord)| {
                    let previous_chord = chord.load(Ordering::Relaxed);

                    let shape = shaper(&ShaperCallbackArgs {
                        chunk_index,
                        num_chunks,
                        index,
                        opening_width: self.opening_width,
                        gap_width: self.gap_width,
                        previous_chord,
                    })
                    .clamp(-1.0, 1.0);
                    chord.store(shape, Ordering::Relaxed);
                });
        }

        self.info()
    }

    pub fn info(&self) -> ChamberInfo {
        ChamberInfo {
            resolution: self.wheel.len() as u32,
            is_left_channel: self.is_left_channel,
            wheel: self
                .wheel
                .iter()
                .map(|chord| chord.load(Ordering::Relaxed))
                .collect(),
            window_size: self.window_size.load(Ordering::Relaxed),
        }
    }
}

impl DspUnit for Chamber {
    fn backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn DspUnitBackend<S> + Send + Sync> {
        let is_left_channel = self.is_left_channel;
        let wheel = self.wheel.clone();
        let window_size = self.window_size.clone();
        let speed = self.speed.clone();
        let mut resonator_input = ResonatorHh::input_frame::<S>();
        let mut resonator_output = ResonatorHh::output_frame::<S>();

        // let (ch_index, num_chambers) = self.pos;
        let mut window_pos = 0_usize;
        let mut resonators_backends: [Box<dyn DspUnitBackend<S> + Send + Sync>; 4] = [
            self.resonators[0].backend::<S>(),
            self.resonators[1].backend::<S>(),
            self.resonators[2].backend::<S>(),
            self.resonators[3].backend::<S>(),
        ];

        Box::new(
            move |tick_data: &NetTickData, input: &[S], output: &mut [S]| {
                let [direct_xct, _adj_xct, left, right] = *input else {
                    log::error!("Chamber input must be a slice of length 4");
                    return;
                };

                let speed = speed.load(Ordering::Relaxed);
                let window_size = window_size.load(Ordering::Relaxed).max(1) as usize;
                let mut opening_area = S::zero();
                let mut perimetry = S::zero();
                let mut prev_chord = S::zero();
                for chord in wheel.iter().cycle().skip(window_pos).take(window_size) {
                    let chord = S::from(chord.load(Ordering::Relaxed)).unwrap_or(S::zero());

                    opening_area = chord + opening_area;
                    perimetry = (chord - prev_chord).abs()
                        + perimetry
                        + if !chord.is_zero() {
                            S::from(2).unwrap()
                        } else {
                            S::zero()
                        };

                    prev_chord = chord;
                }

                let opening_area_gain = opening_area / S::from(window_size).unwrap();
                let perimetry_gain = perimetry / S::from(window_size * 4).unwrap();

                let through_xct = direct_xct * opening_area_gain;
                let output_xct = {
                    let mut resonance_avg = S::zero();
                    resonator_input[0] = through_xct;
                    for res in resonators_backends.iter_mut() {
                        res.process(tick_data, &resonator_input, &mut resonator_output);
                        resonance_avg = resonator_output[0] + resonance_avg;
                    }
                    resonance_avg / S::from(resonators_backends.len()).unwrap()
                };

                let remainder_xct = (direct_xct - through_xct) * perimetry_gain;

                output[0] = direct_xct;
                output[1] = remainder_xct;

                if is_left_channel {
                    output[2] = output_xct + left;
                    output[3] = right;
                } else {
                    output[2] = left;
                    output[3] = output_xct + right;
                }

                window_pos = (window_pos + speed as usize) % wheel.len();
            },
        )
    }

    fn output_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 4]
    }

    fn input_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 4]
    }
}
