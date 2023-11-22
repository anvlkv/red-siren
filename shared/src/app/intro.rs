use crate::{geometry::Rect, instrument, Navigate};
use crux_core::render::Render;
use crux_core::App;
use crux_macros::Effect;
use keyframe::{
    functions::{EaseIn, EaseOut},
    keyframes, AnimationSequence,
};
use keyframe_derive::CanTween;
use mint::{Point2, Point3};
use serde::{Deserialize, Serialize};

const INTRO_DURATION: f64 = 2750.0;

#[derive(Default)]
pub struct Intro;

#[derive(Default)]
pub struct Model {
    layout: instrument::Layout,
    config: instrument::Config,
    sequence: Option<AnimationSequence<IntroVM>>,
    init_view_model: Option<IntroVM>,
    ts_start: f64,
    ts_end: f64,
    ts_current: f64,
    reduced_motion: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, CanTween)]
pub struct IntroVM {
    pub layout: instrument::Layout,
    pub animation_progress: f64,
    pub view_box: Rect,
    pub intro_opacity: f64,
    pub flute_rotation: Point3<f64>,
    pub flute_position: Point2<f64>,
    pub buttons_position: Point2<f64>,
    pub button_size: f64,
}

impl Eq for IntroVM {}

impl Default for IntroVM {
    fn default() -> Self {
        Self {
            layout: instrument::Layout::dummy(4.25253, 282.096, 78.0),
            animation_progress: 0.0,
            view_box: Rect::size(430.0, 932.0),
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

impl IntroVM {
    fn init_size(width: f64, height: f64) -> Self {
        let scale = (430.0 / width).min(932.0 / height);

        Self {
            layout: instrument::Layout::dummy(4.25253 * scale, 282.096 * scale, 78.0 * scale),
            animation_progress: 0.0,
            view_box: Rect::size(width, height),
            intro_opacity: 1.0,
            flute_rotation: Point3 {
                z: -17.1246,
                x: 48.3365 * scale,
                y: 585.964 * scale,
            },
            flute_position: Point2 {
                x: 48.3365 * scale,
                y: 585.964 * scale,
            },
            buttons_position: Point2 {
                x: (107.0 - 39.0) * scale,
                y: (164.0 - 39.0) * scale,
            },
            button_size: 78.0 * scale,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum IntroEV {
    SetInstrumentTarget(instrument::Layout, instrument::Config),
    StartAnimation { ts_start: f64, reduced_motion: bool },
    SetViewBoxInit { width: f64, height: f64 },
    TsNext(f64),
}

impl Eq for IntroEV {}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Intro")]
pub struct IntroCapabilities {
    pub render: Render<IntroEV>,
    pub navigate: Navigate<IntroEV>,
}

impl App for Intro {
    type Event = IntroEV;

    type Model = Model;

    type ViewModel = IntroVM;

    type Capabilities = IntroCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            IntroEV::SetViewBoxInit { width, height } => {
                model.init_view_model = Some(IntroVM::init_size(width, height));
            }
            IntroEV::SetInstrumentTarget(layout, config) => {
                model.layout = layout;
                model.config = config;
                self.update_sequence(model);
                self.update(IntroEV::TsNext(model.ts_current), model, caps);
                caps.render.render();
            }
            IntroEV::StartAnimation {
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
            IntroEV::TsNext(ts) => {
                model.ts_current = ts;
                let seq = model.sequence.as_mut();
                let seq = seq.unwrap();
                let advance_duration = (ts - model.ts_start) / INTRO_DURATION;
                if model.reduced_motion {
                    if seq.has_keyframe_at(advance_duration)
                        || seq
                            .pair()
                            .1
                            .map(|f| f.time() <= advance_duration)
                            .unwrap_or(true)
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

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let seq = &model.sequence;

        if let Some(seq) = seq.as_ref() {
            let now = seq.now();

            IntroVM {
                animation_progress: seq.progress(),
                ..now
            }
        } else {
            IntroVM::default()
        }
    }
}

impl Intro {
    fn update_sequence(&self, model: &mut Model) {
        let init_vm = model.init_view_model.clone().unwrap_or_default();
        let vb_target = Rect::size(model.config.width, model.config.height);

        let flute_position_target = Point2 { x: 0.0, y: 0.0 };
        let flute_rotation_target = Point3 {
            z: 0.0,
            x: 0.0,
            y: 0.0,
        };
        let buttons_position_target = Point2 { x: 0.0, y: 0.0 };
        let target_button_size = model.config.button_size;

        let tracks_intermediate = model.layout.buttons.clone();
        let tracks_intermediate = tracks_intermediate
            .into_iter()
            .map(|b| {
                let (left, right, top, bottom) = b.components();
                let button_track_margin =
                    model.config.button_size * model.config.button_track_margin;
                Rect::new(
                    left - button_track_margin,
                    right + button_track_margin,
                    top - button_track_margin,
                    bottom + button_track_margin,
                )
            })
            .collect::<Vec<_>>();

        let animation: AnimationSequence<IntroVM> = keyframes![
            (IntroVM { ..init_vm.clone() }, 0.0, EaseIn),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    button_size: target_button_size,
                    ..init_vm.clone()
                },
                0.5,
                EaseOut
            ),
            (
                IntroVM {
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
                    ..init_vm.clone()
                },
                0.75,
                EaseOut
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    layout: instrument::Layout {
                        tracks: tracks_intermediate,
                        ..model.layout.clone()
                    },
                    view_box: vb_target,
                    button_size: target_button_size,
                    flute_rotation: flute_rotation_target,
                    flute_position: flute_position_target,
                    buttons_position: buttons_position_target,
                    ..init_vm.clone()
                },
                0.85,
                EaseOut
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    layout: model.layout.clone(),
                    view_box: vb_target,
                    button_size: target_button_size,
                    flute_rotation: flute_rotation_target,
                    flute_position: flute_position_target,
                    buttons_position: buttons_position_target,
                    ..init_vm.clone()
                },
                1.0,
                EaseIn
            )
        ];
        let _ = model.sequence.insert(animation);
    }
}
