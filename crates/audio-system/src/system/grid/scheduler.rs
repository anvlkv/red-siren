use std::{collections::BTreeMap, marker::PhantomData};

use fundsp::{
    prelude::*,
    typenum::{Sum, Unsigned},
};
use num_rational::Rational32;

use crate::system::grid::{bpm_grid::MetroSignal, FrameEncodedSignal};

#[derive(Clone, Copy, Default)]
pub enum SchedulingRequest {
    Request {
        /// The time at which the event should occur, relative to the start of the current beat, in beats.
        time: Rational32,
        /// The duration of the event in beats.
        duration: Rational32,
        /// The number of beats the event should repeat after the first occurrence.
        repeat: Option<u32>,
        /// The event to schedule.
        event: f64,
    },
    #[default]
    None,
}

unsafe impl FrameEncodedSignal for SchedulingRequest {
    type Size = U8; // 32÷4 = 8
}


#[derive(Clone, Copy)]
pub enum SchedulingEvent {
    Event {
        value: f64,
        duration_s: f64
    },
    None
}

unsafe impl FrameEncodedSignal for SchedulingEvent {
    type Size = U6; // 24÷4 = 6
}

#[derive(Clone)]
pub struct Scheduler<F: Real> {
    _sample_type: PhantomData<F>,
    ticks_per_beat: u32,
    ticks_to_next_beat: u32,
    schedule: BTreeMap<u32, SchedulingRequest>,
    sample_rate: f64,
}

impl<F: Real> Scheduler<F> {
    pub fn new() -> Self {
        Self {
            _sample_type: PhantomData,
            ticks_per_beat: 0,
            ticks_to_next_beat: 0,
            sample_rate: DEFAULT_SR,
            schedule: BTreeMap::new(),
        }
    }

    fn tick_next(&mut self) -> Option<SchedulingEvent> {
        let current_tick = self
            .ticks_per_beat
            .checked_sub(self.ticks_to_next_beat)
            .unwrap_or(0);
        self.ticks_to_next_beat = self.ticks_to_next_beat.saturating_sub(1);
        self.schedule
            .remove(&current_tick)
            .and_then(|request| match request {
                SchedulingRequest::Request {
                    time,
                    duration,
                    repeat,
                    event,
                } => {
                    // Schedule the next occurrence of this event if it is repeating.
                    if let Some(repeat) = repeat && repeat > 0 {
                        let next_tick = (time * self.ticks_per_beat as i32).to_integer() as u32;
                        self.schedule.insert(
                            next_tick,
                            SchedulingRequest::Request {
                                time,
                                duration,
                                repeat: Some(repeat - 1),
                                event,
                            },
                        );
                    }

                    let duration_s =  {
                        let event_duration_s = (duration * self.ticks_per_beat as i32) / self.sample_rate as i32;
                    
                        *event_duration_s.numer() as f64 / *event_duration_s.denom() as f64 
                    };

                    Some(SchedulingEvent::Event { value: event, duration_s})
                }
                SchedulingRequest::None => None,
            })
    }

    fn schedule_event(&mut self, request: SchedulingRequest) {
        if let SchedulingRequest::Request { time, .. } = request {
            let tick = (time * self.ticks_per_beat as i32).to_integer() as u32;
            self.schedule.insert(tick, request);
        }
    }

    fn update_tempo(&mut self, new_ticks_per_beat: u32) {
        let old_ticks_per_beat = self.ticks_per_beat;
        let ticks_to_next_beat = (self.ticks_to_next_beat as f64 * new_ticks_per_beat as f64 / old_ticks_per_beat as f64) as u32;
        self.ticks_to_next_beat = ticks_to_next_beat;
        self.ticks_per_beat = new_ticks_per_beat;
        // Reschedule all events in the schedule according to the new tempo.
        let mut new_schedule = BTreeMap::new();
        for (_, request) in self.schedule.iter() {
            if let SchedulingRequest::Request { time, .. } = request {
                let new_tick = (time * self.ticks_per_beat as i32).to_integer() as u32;
                new_schedule.insert(new_tick, *request);
            }
        }
        self.schedule = new_schedule;
    }
}

impl<F: Real> AudioNode for Scheduler<F> {
    const ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Scheduler"));

    type Inputs = Sum<
        <MetroSignal as FrameEncodedSignal>::Size,
        <SchedulingRequest as FrameEncodedSignal>::Size,
    >;

    type Outputs = <SchedulingEvent as FrameEncodedSignal>::Size;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let output = self.tick_next().unwrap_or(SchedulingEvent::None);
        let metro_slice = &input[..<MetroSignal as FrameEncodedSignal>::Size::USIZE];
        let request_slice = &input[<MetroSignal as FrameEncodedSignal>::Size::USIZE..];

        let metro_signal = MetroSignal::decode(&Frame::from_slice(metro_slice));
        let scheduling_request = SchedulingRequest::decode(&Frame::from_slice(request_slice));

        match metro_signal {
            MetroSignal::TicksToNextBeat(ttb) => {
                self.ticks_to_next_beat = ttb;
            }
            MetroSignal::Reschedule(new_ttb) => {
                self.update_tempo(new_ttb);
            }
            MetroSignal::None => {}
        }

        match scheduling_request {
            SchedulingRequest::Request { .. } => self.schedule_event(scheduling_request),
            SchedulingRequest::None => {}
        }
        
        output.encode()
    }
}
