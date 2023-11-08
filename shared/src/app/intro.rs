use crux_core::render::Render;
use crux_core::App;
use crux_kv::KeyValue;
use crux_macros::Effect;
use keyframe::{
    functions::{EaseIn, EaseOut},
    keyframes, AnimationSequence,
};
use keyframe_derive::CanTween;
use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use crate::instrument;

const INTRO_DURATION: f64 = 1700.0;

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
}

#[derive(Serialize, Deserialize, Clone, CanTween, PartialEq, Copy)]
pub struct ViewModel {
    pub intro_opacity: f64,
    pub flute_rotation: f64,
    pub flute_scale: f64,
    pub flute_stroke: f64,
    pub button_size: f64,
    pub buttons_group: f64,
    pub groups: f64,
    pub button_gap: f64,
    pub button_group_gap: f64,
    pub buttons_position_offset: Point2<f64>,
    pub flute_position_offset: Point2<f64>,
    pub view_box: Vector2<Point2<f64>>,
}

impl Hash for ViewModel {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.intro_opacity.to_be_bytes().hash(state);
        self.flute_rotation.to_be_bytes().hash(state);
        self.flute_scale.to_be_bytes().hash(state);
        self.flute_stroke.to_be_bytes().hash(state);
        self.button_size.to_be_bytes().hash(state);
        self.buttons_group.to_be_bytes().hash(state);
        self.button_gap.to_be_bytes().hash(state);
        self.button_group_gap.to_be_bytes().hash(state);
        self.buttons_position_offset.x.to_be_bytes().hash(state);
        self.buttons_position_offset.y.to_be_bytes().hash(state);
        self.flute_position_offset.x.to_be_bytes().hash(state);
        self.flute_position_offset.y.to_be_bytes().hash(state);
        self.view_box.x.x.to_be_bytes().hash(state);
        self.view_box.x.y.to_be_bytes().hash(state);
        self.view_box.y.x.to_be_bytes().hash(state);
        self.view_box.y.y.to_be_bytes().hash(state);
    }
}

impl Eq for ViewModel {}

impl Default for ViewModel {
    fn default() -> Self {
        Self {
            intro_opacity: 1.0,
            buttons_group: 1.0,
            groups: 1.0,
            // zero
            flute_rotation: 0.0,
            flute_scale: 0.0,
            flute_stroke: 2.0,
            button_size: 78.0,
            button_gap: 0.0,
            button_group_gap: 0.0,
            buttons_position_offset: Point2 { x: 0.0, y: 0.0 },
            flute_position_offset: Point2 { x: 0.0, y: 0.0 },
            view_box: Vector2 {
                x: Point2 { x: 0.0, y: 0.0 },
                y: Point2 { x: 430.0, y: 932.0 },
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Event {
    SetInstrumentTarget(instrument::Config),
    StartAnimation {
        ts_start: f64,
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
                log::debug!("Event::SetInstrumentTarget, {:?}", config);
                model.config = config;

                self.update_sequence(model);
            }
            Event::StartAnimation { ts_start, config } => {
                log::debug!("Event::StartAnimation, {} {:?}", ts_start, config);
                model.config = config;
                model.ts_start = ts_start;
                model.ts_end = ts_start + INTRO_DURATION;
                model.ts_current = ts_start;

                self.update_sequence(model);
                caps.render.render();
            }
            Event::TsNext(ts) => {
                log::debug!("Event::TsNext, {}", ts);
                model.ts_current = ts;
                let seq = self.sequence.clone();
                let mut seq = seq.lock().unwrap();
                let seq = seq.as_mut().unwrap();
                let advance_duration = (ts - model.ts_start) / INTRO_DURATION;
                seq.advance_to(advance_duration);
                caps.render.render();
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
                y: model.config.height as f64
            }
        };
        let animation: AnimationSequence<ViewModel> = keyframes![
            (ViewModel::default(), 0.0, EaseIn),
            (
                ViewModel {
                    intro_opacity: 0.0,
                    button_size: model.config.button_size as f64,
                    view_box: vb_target,
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
                    ..ViewModel::default()
                },
                1.0
            )
        ];
        let _ = seq.insert(animation);
    }
}
