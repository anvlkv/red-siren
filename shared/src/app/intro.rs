use crux_core::render::Render;
use crux_core::App;
use crux_kv::KeyValue;
use crux_macros::Effect;
use keyframe::{
    functions::{EaseIn, EaseOut},
    keyframes, AnimationSequence,
};
use keyframe_derive::CanTween;
use mint::{Point2, Point3, Vector2};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::instrument;

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

#[derive(Default, Serialize, Deserialize)]
pub struct Model {
    config: instrument::Config,
    ts_start: f64,
    ts_end: f64,
    ts_current: f64,
    reduced_motion: bool,
}

#[derive(Serialize, Deserialize, Clone, CanTween, PartialEq, Copy)]
pub struct ViewModel {
    pub intro_opacity: f64,
    pub flute_rotation: Point3<f64>,
    pub buttons_rotation: Point3<f64>,
    pub flute_scale: f64,
    pub flute_stroke: f64,
    pub button_size: f64,
    pub buttons_group: f64,
    pub groups: f64,
    pub button_gap: f64,
    pub button_group_gap: f64,
    pub buttons_position: Point2<f64>,
    pub flute_position: Point2<f64>,
    pub flute_size: Point2<f64>,
    pub view_box: Vector2<Point2<f64>>,
    pub track_size: Point2<f64>,
    pub track_radius: f64,
    pub tracks_offset: f64,
}

impl Eq for ViewModel {}

impl Default for ViewModel {
    fn default() -> Self {
        Self {
            intro_opacity: 1.0,
            buttons_group: 1.0,
            groups: 1.0,
            flute_rotation: Point3 {
                z: -17.1246,
                x: 0.0,
                y: 0.0,
            },
            flute_position: Point2 {
                x: 48.3365,
                y: 585.964,
            },
            flute_size: Point2 {
                x: 282.096,
                y: 4.25253,
            },
            flute_scale: 1.0,
            flute_stroke: 2.0,
            button_size: 78.0,
            button_gap: 0.0,
            button_group_gap: 0.0,
            buttons_position: Point2 {
                x: 107.0 - 39.0,
                y: 164.0 - 39.0,
            },
            buttons_rotation: Point3 {
                z: 0.0,
                x: 0.0,
                y: 0.0,
            },
            view_box: Vector2 {
                x: Point2 { x: 0.0, y: 0.0 },
                y: Point2 { x: 430.0, y: 932.0 },
            },

            track_radius: 0.0,
            track_size: Point2 { x: 0.0, y: 0.0 },
            tracks_offset: 0.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Event {
    SetInstrumentTarget(instrument::Config),
    StartAnimation {
        ts_start: f64,
        reduced_motion: bool,
        config: instrument::Config,
    },
    TsNext(f64),
}

impl Eq for Event {}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Intro")]
pub struct IntroCapabilities {
    pub render: Render<Event>,
    pub key_value: KeyValue<Event>,
}

impl App for Intro {
    type Event = Event;

    type Model = Model;

    type ViewModel = ViewModel;

    type Capabilities = IntroCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            Event::SetInstrumentTarget(config) => {
                model.config = config;

                self.update_sequence(model);
            }
            Event::StartAnimation {
                ts_start,
                config,
                reduced_motion,
            } => {
                model.config = config;
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
                caps.render.render();

                // if (seq.finished()) {
                //     caps.key_value.write("animation_ended", true, ||{})
                // }
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let seq = self.sequence.clone();
        let seq = seq.lock().unwrap();

        if let Some(seq) = seq.as_ref() {
            seq.now()
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
                x: model.config.width as f64,
                y: model.config.height as f64,
            },
        };

        let flute_size = Point2 {
            x: model.config.length as f64 + model.config.gap.0 as f64 * 2.0,
            y: model.config.breadth as f64,
        };

        let flute_position = if model.config.portrait {
            Point2 {
                x: (model.config.width as f64) - flute_size.y,
                y: -1.0 * model.config.gap.0 as f64,
            }
        } else {
            Point2 {
                x: (model.config.width + model.config.gap.0 as u16) as f64,
                y: (model.config.height as f64) - flute_size.y,
            }
        };

        let flute_rotation = if model.config.portrait {
            Point3 {
                z: 90.0,
                y: 0.0,
                x: 0.0,
            }
        } else {
            Point3 {
                z: 180.0,
                y: 0.0,
                x: 0.0,
            }
        };
        let buttons_rotation = Point3 {
            z: flute_rotation.z - 90.0,
            y: 0.0,
            x: 0.0,
        };

        let buttons_position = if model.config.portrait {
            Point2 {
                x: (model.config.width as f64 - model.config.button_size as f64) / 2.0,
                y: model.config.gap.1 as f64 / 2.0,
            }
        } else {
            Point2 {
                x: (model.config.height as f64 - model.config.button_size as f64) / 2.0,
                y: -1.0 * model.config.width as f64 + model.config.gap.1 as f64,
            }
        };

        let track_radius = (model.config.button_size + model.config.gap.0 / 2)as f64 / 2.0;

        let track_size = Point2 {
            x: model.config.breadth as f64 * 3.0 / 2.0 + model.config.gap.0 as f64 / 2.0 + track_radius,
            y: model.config.button_size as f64 + model.config.gap.0 as f64 / 2.0,
        };
        let tracks_offset = (model.config.buttons_group as usize
            * model.config.groups as usize) as f64 * track_size.x;

        let animation: AnimationSequence<ViewModel> = keyframes![
            (ViewModel::default(), 0.0, EaseIn),
            (
                ViewModel {
                    intro_opacity: 0.0,
                    flute_rotation: Point3 {
                        z: 0.0,
                        x: 48.3365,
                        y: 585.964
                    },
                    ..ViewModel::default()
                },
                0.5,
                EaseOut
            ),
            (
                ViewModel {
                    intro_opacity: 0.0,
                    button_size: model.config.button_size as f64,
                    buttons_group: model.config.buttons_group as f64,
                    groups: model.config.groups as f64,
                    button_gap: model.config.gap.0 as f64,
                    button_group_gap: model.config.gap.1 as f64,
                    view_box: vb_target,
                    flute_rotation,
                    flute_size,
                    flute_position,
                    buttons_position,
                    buttons_rotation,
                    flute_stroke: 1.0,
                    tracks_offset,
                    track_radius,
                    track_size: Point2 {
                        y: track_size.y,
                        x: 0.0
                    },
                    ..ViewModel::default()
                },
                0.75,
                EaseIn
            ),
            (
                ViewModel {
                    intro_opacity: 0.0,
                    button_size: model.config.button_size as f64,
                    buttons_group: model.config.buttons_group as f64,
                    groups: model.config.groups as f64,
                    button_gap: model.config.gap.0 as f64,
                    button_group_gap: model.config.gap.1 as f64,
                    view_box: vb_target,
                    flute_rotation,
                    flute_size,
                    flute_position,
                    buttons_position,
                    buttons_rotation,
                    flute_stroke: 1.0,
                    track_radius,
                    track_size,
                    ..ViewModel::default()
                },
                1.0,
                EaseOut
            )
        ];
        let _ = seq.insert(animation);
    }
}
