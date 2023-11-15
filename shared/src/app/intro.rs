use crux_core::render::Render;
use crux_core::App;
use crux_macros::Effect;
use keyframe::{
    functions::{EaseIn, EaseOut},
    keyframes, AnimationSequence,
};
use keyframe_derive::CanTween;
use mint::{Point2, Point3, Vector2};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::{instrument, Navigate};

const INTRO_DURATION: f64 = 2750.0;

pub struct Intro {
    sequence: Arc<Mutex<Option<AnimationSequence<ViewModel>>>>,
}

impl Default for Intro {
    fn default() -> Self {
        Self {
            sequence: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Default)]
pub struct Model {
    layout: instrument::Layout,
    config: instrument::Config,
    ts_start: f64,
    ts_end: f64,
    ts_current: f64,
    reduced_motion: bool,
}

#[derive(Serialize, Deserialize, Clone, CanTween, PartialEq)]
pub struct ViewModel {
    pub animation_progress: f64,
    pub view_box: Vector2<Point2<f32>>,
    pub intro_opacity: f32,
    pub layout: instrument::Layout,
    pub flute_rotation: Point3<f32>,
    pub flute_position: Point2<f32>,
    pub buttons_position: Point2<f32>,
    pub button_size: f32,
}

impl Eq for ViewModel {}

impl Default for ViewModel {
    fn default() -> Self {
        Self {
            layout: instrument::Layout::dummy(4.25253, 282.096, 78.0),
            animation_progress: 0.0,
            view_box: Vector2 {
                x: Point2 { x: 0.0, y: 0.0 },
                y: Point2 { x: 430.0, y: 932.0 },
            },
            intro_opacity: 1.0,
            flute_rotation: Point3 {
                z: -17.1246,
                x: 48.3365,
                y: 585.964,
            },
            flute_position: Point2 {
                x: 48.3365,
                y: 585.964,
            },
            buttons_position: Point2 {
                x: 107.0 - 39.0,
                y: 164.0 - 39.0,
            },
            button_size: 78.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Event {
    SetInstrumentTarget(instrument::Layout, instrument::Config),
    StartAnimation { ts_start: f64, reduced_motion: bool },
    TsNext(f64),
}

impl Eq for Event {}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Intro")]
pub struct IntroCapabilities {
    pub render: Render<Event>,
    pub navigate: Navigate<Event>,
}

impl App for Intro {
    type Event = Event;

    type Model = Model;

    type ViewModel = ViewModel;

    type Capabilities = IntroCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            Event::SetInstrumentTarget(layout, config) => {
                model.layout = layout;
                model.config = config;
                self.update_sequence(model);
                self.update(Event::TsNext(model.ts_current), model, caps);
                caps.render.render();
            }
            Event::StartAnimation {
                ts_start,
                reduced_motion,
            } => {
                model.ts_start = ts_start;
                model.ts_end = ts_start + INTRO_DURATION;
                model.ts_current = ts_start;
                model.reduced_motion = reduced_motion;

                self.update_sequence(model);
                caps.render.render();
            }
            Event::TsNext(ts) => {
                model.ts_current = ts;
                let seq = self.sequence.clone();
                let mut seq = seq.lock().unwrap();
                let seq = seq.as_mut().unwrap();
                let advance_duration = (ts - model.ts_start) / INTRO_DURATION;
                if model.reduced_motion {
                    if seq.has_keyframe_at(advance_duration)
                        || seq
                            .pair()
                            .1
                            .map(|f| f.time() < advance_duration)
                            .unwrap_or(false)
                    {
                        seq.advance_to(advance_duration);
                    }
                } else {
                    seq.advance_to(advance_duration);
                }

                if !seq.finished() {
                    caps.render.render();
                } else {
                    caps.navigate.to(crate::Activity::Play)
                }
            }
        }
    }

    fn view(&self, _model: &Self::Model) -> Self::ViewModel {
        let seq = self.sequence.clone();
        let seq = seq.lock().unwrap();

        if let Some(seq) = seq.as_ref() {
            let now = seq.now();

            ViewModel {
                animation_progress: seq.progress(),
                ..now
            }
        } else {
            ViewModel::default()
        }
    }
}

impl Intro {
    fn update_sequence(&self, model: &Model) {
        let seq = self.sequence.clone();
        let mut seq = seq.lock().unwrap();
        let vb_target = Vector2 {
            x: Point2 { x: 0.0, y: 0.0 },
            y: Point2 {
                x: model.config.width,
                y: model.config.height,
            },
        };
        let flute_position_target = Point2 { x: 0.0, y: 0.0 };
        let flute_rotation_target = Point3 {
            z: 0.0,
            x: 0.0,
            y: 0.0,
        };
        let buttons_position_target = Point2 { x: 0.0, y: 0.0 };
        let target_button_size = model.config.button_size;

        let animation: AnimationSequence<ViewModel> = keyframes![
            (
                ViewModel {
                    ..ViewModel::default()
                },
                0.0,
                EaseIn
            ),
            (
                ViewModel {
                    intro_opacity: 0.0,
                    button_size: target_button_size,
                    ..ViewModel::default()
                },
                0.5,
                EaseOut
            ),
            (
                ViewModel {
                    intro_opacity: 0.0,
                    layout: instrument::Layout {
                        tracks: vec![],
                        ..model.layout.clone()
                    },
                    view_box: vb_target,
                    button_size: target_button_size,
                    flute_rotation: flute_rotation_target,
                    flute_position: flute_position_target,
                    buttons_position: buttons_position_target,
                    ..ViewModel::default()
                },
                0.75,
                EaseOut
            ),
            (
                ViewModel {
                    intro_opacity: 0.0,
                    layout: model.layout.clone(),
                    view_box: vb_target,
                    button_size: target_button_size,
                    flute_rotation: flute_rotation_target,
                    flute_position: flute_position_target,
                    buttons_position: buttons_position_target,
                    ..ViewModel::default()
                },
                1.0,
                EaseIn
            )
        ];
        let _ = seq.insert(animation);
    }
}
