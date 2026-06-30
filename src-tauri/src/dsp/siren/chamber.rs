use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use atomic_float::AtomicF64;
use num_traits::Float;

use crate::dsp::{DspUnit, DspUnitBackend, NetTickData};

pub struct Chamber {
    pub is_left_channel: bool,
    pub wheel: Arc<Vec<AtomicF64>>,
    pub window_size: Arc<AtomicU32>,
    pub speed: Arc<AtomicU32>,
    pub space_samples: Arc<AtomicU32>,
    pub pos: (usize, usize),
    pub opening_width: usize,
    pub gap_width: usize,
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
        pos: (usize, usize),
        is_left_channel: bool,
        initial_window_size: u32,
        initial_space_samples: u32,
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
        let space_samples = Arc::new(AtomicU32::new(initial_space_samples));

        Self {
            is_left_channel,
            wheel,
            window_size,
            speed,
            space_samples,
            pos,
            opening_width,
            gap_width,
        }
    }

    pub fn shape<F>(&self, mut shaper: F)
    where
        F: FnMut(&ShaperCallbackArgs) -> f64,
    {
        let num_chunks = self.wheel.len() / (self.opening_width + self.gap_width);

        for (chunk_index, chunk) in self
            .wheel
            .chunks(self.opening_width + self.gap_width)
            .enumerate()
        {
            if chunk.len() < self.opening_width {
                continue;
            }
            chunk[..self.opening_width]
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
        let space_samples = self.space_samples.clone();
        let (ch_index, num_chambers) = self.pos;
        let initial_size = space_samples.load(Ordering::Relaxed) as usize;
        let mut window_pos = 0_usize;
        let mut space: Vec<S> = vec![S::zero(); initial_size];

        Box::new(
            move |_tick_data: &NetTickData, input: &[S], output: &mut [S]| {
                let [direct_xct, _adj_xct, left, right] = *input else {
                    log::error!("Chamber input must be a slice of length 4");
                    return;
                };

                let speed = speed.load(Ordering::Relaxed);
                let window_size = window_size.load(Ordering::Relaxed).max(1) as usize;
                // let space_samples = space_samples.load(Ordering::Relaxed) as usize;
                let mut opening_area = S::zero();
                // let mut interaction_area = S::zero();
                for chord in wheel.iter().cycle().skip(window_pos).take(window_size) {
                    let chord = S::from(chord.load(Ordering::Relaxed)).unwrap_or(S::zero());

                    if chord > S::zero() {
                        opening_area = chord + opening_area;
                    }

                    // interaction_area = (S::one() - chord) + interaction_area;
                }

                let opening_area_gain = opening_area / S::from(window_size).unwrap();
                // let interaction_area_gain = interaction_area / S::from(window_size).unwrap();

                // let space_xct = if !space.is_empty() {
                //     space.rotate_left(1);
                //     space[1..].iter_mut().enumerate().for_each(|(at, x)| {
                //         let s_pos = S::from(space_samples - at + 1).unwrap_or(S::one());
                //         *x = *x - (S::one() / (s_pos + s_pos * adj_xct)) * *x;
                //     });
                //     space[0] + space[0] * adj_xct
                // } else {
                //     S::zero()
                // };

                // if space.len() != space_samples {
                //     space.resize(space_samples, S::zero());
                // }

                let through_xct = direct_xct * opening_area_gain;

                let output_xct = through_xct; // + space_xct;
                                              // if let Some(last_mut) = space.last_mut() {
                                              //     *last_mut = output_xct;
                                              // }

                // let remainder_xct = (direct_xct - through_xct) * interaction_area_gain;

                // let pos_gain = S::one() / S::from(num_chambers - ch_index).unwrap();

                output[0] = through_xct;
                // output[1] = remainder_xct;

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
