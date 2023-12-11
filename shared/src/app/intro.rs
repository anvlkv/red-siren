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

use crate::instrument::layout::MenuPosition;
use crate::{geometry::Rect, instrument, Activity, Navigate};

const INTRO_DURATION: f64 = 2750.0;
const EXIT_DURATION: f64 = 750.0;

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
    intro_animation_ended: bool,
    transition_animation_started: bool,
    transition_to: Option<Activity>,
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
    pub menu_opacity: f64,
}

impl Eq for IntroVM {}

impl Default for IntroVM {
    fn default() -> Self {
        Self {
            layout: instrument::Layout::dummy(4.25253, 282.096, 78.0),
            animation_progress: 0.0,
            view_box: Rect::size(430.0, 932.0),
            intro_opacity: 1.0,
            menu_opacity: 0.0,
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
            menu_opacity: 0.0,
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
    SetInstrumentTarget(Box<instrument::Layout>, Box<instrument::Config>),
    StartAnimation { ts_start: f64, reduced_motion: bool },
    SetViewBoxInit { width: f64, height: f64 },
    TsNext(f64),
    Menu(Activity),
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
                model.layout = *layout;
                model.config = *config;
                model.intro_animation_ended = false;
                self.build_intro_sequence(model);
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
                model.intro_animation_ended = false;
                self.build_intro_sequence(model);
                caps.render.render();
            }
            IntroEV::TsNext(ts) => {
                model.ts_current = ts;
                let advance_duration = (ts - model.ts_start)
                    / if model.transition_animation_started {
                        EXIT_DURATION
                    } else {
                        INTRO_DURATION
                    };

                if !model.intro_animation_ended || model.transition_animation_started {
                    let seq = model.sequence.as_mut();
                    let seq = seq.unwrap();
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

                    if seq.finished() {
                        if model.transition_animation_started {
                            caps.navigate.to(model.transition_to.unwrap())
                        } else {
                            model.intro_animation_ended = true;
                        }
                    }

                    caps.render.render();
                }
            }
            IntroEV::Menu(next_activity) => {
                match &next_activity {
                    Activity::Play => {}
                    Activity::About => {
                        todo!("implement layout")
                    }
                    Activity::Listen => {
                        todo!("implement layout")
                    }
                    Activity::Tune => {
                        todo!("implement layout")
                    }
                    Activity::Intro => unreachable!("already in intro"),
                }
                _ = model.transition_to.insert(next_activity);
                model.transition_animation_started = true;
                self.build_transition_sequence(model);
                model.ts_start = model.ts_current;

                caps.render.render()
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
    fn build_transition_sequence(&self, model: &mut Model) {
        if let Some(seq) = model.sequence.take() {
            let vm = seq.now();

            let transition = keyframes![
                (vm.clone(), 0.0),
                (
                    IntroVM {
                        layout: model.layout.clone(),
                        ..vm.clone()
                    },
                    1.0,
                    EaseOut
                )
            ];

            _ = model.sequence.insert(transition)
        } else {
            log::warn!("no sequence to transition from")
        }
    }
    fn build_intro_sequence(&self, model: &mut Model) {
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

        let off_screen_menu_position = MenuPosition::Center(Rect::new(
            0.0,
            vb_target.width(),
            vb_target.height(),
            vb_target.height() * 2.0,
        ));

        let menu_offset_main = model.config.length / 5.0 * -1.0;
        let menu_offset_side = model.config.breadth / 5.0 * -1.0;
        let target_menu_position = if model.config.portrait {
            MenuPosition::Center(
                vb_target
                    .clone()
                    .offset_top_and_bottom(menu_offset_main, menu_offset_main)
                    .offset_left_and_right(menu_offset_side, menu_offset_side),
            )
        } else {
            MenuPosition::Center(
                vb_target
                    .clone()
                    .offset_top_and_bottom(menu_offset_side, menu_offset_side)
                    .offset_left_and_right(menu_offset_main, menu_offset_main),
            )
        };

        let dummy_layout = instrument::Layout {
            menu_position: off_screen_menu_position.clone(),
            ..IntroVM::default().layout
        };

        let intermediate_layout = instrument::Layout {
            menu_position: off_screen_menu_position.clone(),
            tracks: vec![],
            ..model.layout.clone()
        };

        let animation: AnimationSequence<IntroVM> = keyframes![
            (
                IntroVM {
                    layout: dummy_layout.clone(),
                    ..init_vm.clone()
                },
                0.0,
                EaseIn
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    button_size: target_button_size,
                    layout: dummy_layout.clone(),
                    ..init_vm.clone()
                },
                0.25,
                EaseOut
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    layout: intermediate_layout.clone(),
                    view_box: vb_target,
                    button_size: target_button_size,
                    flute_rotation: flute_rotation_target,
                    flute_position: flute_position_target,
                    buttons_position: buttons_position_target,
                    ..init_vm.clone()
                },
                0.5,
                EaseOut
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    layout: instrument::Layout {
                        tracks: tracks_intermediate,
                        ..intermediate_layout.clone()
                    },
                    view_box: vb_target,
                    button_size: target_button_size,
                    flute_rotation: flute_rotation_target,
                    flute_position: flute_position_target,
                    buttons_position: buttons_position_target,
                    ..init_vm.clone()
                },
                0.65,
                EaseOut
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    layout: instrument::Layout {
                        menu_position: off_screen_menu_position.clone(),
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
                EaseIn
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    layout: instrument::Layout {
                        menu_position: target_menu_position.clone(),
                        ..model.layout.clone()
                    },
                    menu_opacity: 1.0,
                    view_box: vb_target,
                    button_size: target_button_size,
                    flute_rotation: flute_rotation_target,
                    flute_position: flute_position_target,
                    buttons_position: buttons_position_target,
                    ..init_vm.clone()
                },
                1.0,
                EaseOut
            )
        ];
        _ = model.sequence.insert(animation);
    }
}
