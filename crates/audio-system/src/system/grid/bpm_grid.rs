use fundsp::prelude::*;

use crate::system::grid::FrameEncodedSignal as _;

const BPM_GRID_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::BpmGrid"));

#[derive(Clone, Copy)]
pub enum MetroSignal {
    TicksToNextBeat(u32),
    Reschedule(u32),
    None,
}

unsafe impl super::FrameEncodedSignal for MetroSignal {
    type Size = U2;
}

#[derive(Clone)]
pub struct BpmGrid<F: Real> {
    bpm: F,
    sample_rate: f64,
    ticks_per_beat: u32,
    ticks_to_next_beat: u32,
}

impl<F: Real> AudioNode for BpmGrid<F> {
    const ID: u64 = BPM_GRID_ID;

    type Inputs = U1;

    type Outputs = <MetroSignal as super::FrameEncodedSignal>::Size;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let input_bpm: F = convert(input[0]);

        if input_bpm != self.bpm {
            let old_bpm = self.bpm;
            self.bpm = input_bpm;
            self.ticks_per_beat = (self.sample_rate * 60.0 / self.bpm.to_f64()) as u32;
            self.ticks_to_next_beat = (self.ticks_to_next_beat as f64 * self.sample_rate
                / old_bpm.to_f64()
                / self.sample_rate) as u32;
            MetroSignal::Reschedule(self.ticks_to_next_beat).encode()
        } else if self.ticks_to_next_beat == 0 {
            self.ticks_to_next_beat = self.ticks_per_beat;
            MetroSignal::TicksToNextBeat(self.ticks_per_beat).encode()
        } else {
            self.ticks_to_next_beat -= 1;
            MetroSignal::None.encode()
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let old_sample_rate = self.sample_rate;
        self.sample_rate = sample_rate;
        self.ticks_per_beat = (self.sample_rate * 60.0 / self.bpm.to_f64()) as u32;
        self.ticks_to_next_beat =
            (self.ticks_to_next_beat as f64 * self.sample_rate / old_sample_rate) as u32;
    }
}
