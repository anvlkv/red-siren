use std::fmt::Debug;

use crate::geometry::{Line, Rect};
use crate::instrument::layout::MenuPosition;
use crate::intro::{IntroVM, Model};
use crate::{instrument, Activity};
use keyframe::functions::{EaseIn, EaseOut};
use keyframe::{keyframes, AnimationSequence};
use mint::{Point2, Point3};

const INTRO_DURATION: f64 = 2750.0;
const EXIT_DURATION: f64 = 750.0;

pub struct IntroAnimation {
    pub animation: Animation,
    pub duration: f64,
    pub running: Option<(f64, f64)>,
}

impl IntroAnimation {
    pub fn new(model: &Model, to: Activity) -> Self {
        log::debug!("new transition: {:?} -> {:?}", model.current_activity, to);

        match (model.current_activity, to) {
            (Activity::Intro, Activity::Intro) => Self {
                animation: Animation::loading_intro(model),
                running: None,
                duration: INTRO_DURATION,
            },
            (_, Activity::About) => Self {
                animation: Animation::menu_intro(model, false),
                running: None,
                duration: EXIT_DURATION,
            },
            (Activity::About, _) => Self {
                animation: Animation::menu_intro(model, true),
                running: None,
                duration: EXIT_DURATION,
            },
            (_, Activity::Play) => Self {
                animation: Animation::play_intro(model, false),
                running: None,
                duration: EXIT_DURATION,
            },
            (Activity::Play, _) => Self {
                animation: Animation::play_intro(model, true),
                running: None,
                duration: EXIT_DURATION,
            },
            (_, Activity::Tune) => Self {
                animation: Animation::tuner_intro(model, false),
                running: None,
                duration: EXIT_DURATION,//INTRO_DURATION,
            },
            (Activity::Tune, Activity::Intro) => Self {
                animation: Animation::tuner_intro(model, true),
                running: None,
                duration: EXIT_DURATION,
            },
            _ => {
                todo!()
            }
        }
    }

    pub fn update_to_match(&mut self, model: &Model, activity: Activity) {
        let animation = Self::new(model, activity).animation;
        self.animation = animation;
        log::debug!(
            "update animation to match activity. running {:?}",
            self.running
        );

        if let Some((_, ts)) = self.running {
            _ = self.tick(ts, false);
        }
    }

    pub fn tick(&mut self, ts: f64, reduced_motion: bool) -> bool {
        let (start, now) = self.running.get_or_insert((ts, ts));
        *now = ts;

        let advance_duration = (ts - *start) / self.duration;

        let seq = self.animation.sequence_mut();
        if reduced_motion {
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

        seq.finished()
    }

    pub fn now(&self) -> IntroVM {
        self.animation.sequence().now()
    }

    pub fn progress(&self) -> f64 {
        self.animation.sequence().progress()
    }
}

pub enum Animation {
    LoadingIntro(AnimationSequence<IntroVM>),
    PlayInto(AnimationSequence<IntroVM>),
    MenuIntro(AnimationSequence<IntroVM>),
    TunerIntro(AnimationSequence<IntroVM>),
}

impl Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadingIntro(_) => f.write_str("LoadingIntro"),
            Self::PlayInto(_) => f.write_str("PlayIntro"),
            Self::MenuIntro(_) => f.write_str("MenuIntro"),
            Self::TunerIntro(_) => f.write_str("TunerIntro"),
        }
    }
}

impl Animation {
    fn sequence_mut(&mut self) -> &mut AnimationSequence<IntroVM> {
        match self {
            Animation::LoadingIntro(s)
            | Animation::PlayInto(s)
            | Animation::MenuIntro(s)
            | Animation::TunerIntro(s) => s,
        }
    }

    fn sequence(&self) -> &AnimationSequence<IntroVM> {
        match self {
            Animation::LoadingIntro(s)
            | Animation::PlayInto(s)
            | Animation::MenuIntro(s)
            | Animation::TunerIntro(s) => s,
        }
    }

    fn central_menu_position(model: &Model) -> MenuPosition {
        let vb_target = Rect::size(model.config.width, model.config.height);

        let menu_offset_main = model.config.length / 5.0 * -1.0;
        let menu_offset_side = model.config.breadth / 5.0 * -1.0;
        if model.config.portrait {
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
        }
    }

    fn final_layout(model: &Model) -> IntroVM {
        let init_vm = IntroVM::default();
        let vb_target = Rect::size(model.config.width, model.config.height);

        IntroVM {
            intro_opacity: 0.0,
            menu_opacity: 1.0,
            view_box: vb_target,
            button_size: model.config.button_size,
            flute_rotation: Point3::from([0.0, 0.0, 0.0]),
            flute_position: Point2::from([0.0, 0.0]),
            buttons_position: Point2::from([0.0, 0.0]),
            layout: model.layout.clone(),
            ..init_vm.clone()
        }
    }

    pub fn menu_intro(model: &Model, reverse: bool) -> Self {
        let target_menu_position = Self::central_menu_position(model);
        let mut transition = keyframes![
            (
                IntroVM {
                    menu_flip: 0.0,
                    layout: instrument::Layout {
                        menu_position: target_menu_position.clone(),
                        ..Self::final_layout(model).layout
                    },
                    ..Self::final_layout(model)
                },
                0.0
            ),
            (
                IntroVM {
                    menu_flip: 180.0,
                    menu_opacity: 1.0,
                    layout: instrument::Layout {
                        menu_position: target_menu_position.clone(),
                        ..Self::final_layout(model).layout
                    },
                    ..Default::default()
                },
                1.0
            )
        ];

        if reverse {
            transition.reverse();
        }

        Self::MenuIntro(transition)
    }

    pub fn play_intro(model: &Model, reverse: bool) -> Self {
        let init_menu_position = Self::central_menu_position(model);

        let vm = Self::final_layout(model);

        let mut transition = keyframes![
            (
                IntroVM {
                    layout: instrument::Layout {
                        menu_position: init_menu_position.clone(),
                        ..vm.layout.clone()
                    },
                    ..vm.clone()
                },
                0.0
            ),
            (
                IntroVM {
                    menu_flip: 180.0,
                    ..vm.clone()
                },
                1.0,
                EaseOut
            )
        ];

        if reverse {
            transition.reverse();
        }

        Self::PlayInto(transition)
    }

    // pub fn pairs_from_buttons(model: &Model) -> Vec<Rect> {
    //     if model.config.portrait {
    //         model
    //             .layout
    //             .buttons
    //             .iter()
    //             .enumerate()
    //             .map(|(i, _)| {

    //             })
    //             .collect()
    //     } else {
    //         model.layout.buttons.clone()
    //     }
    // }

    pub fn tuner_intro(model: &Model, reverse: bool) -> Self {
        let init_menu_position = Self::central_menu_position(model);
        let menu_button_position = MenuPosition::TopLeft(
            Rect::size(64.0, 64.0)
                .offset_left(model.config.safe_area[0])
                .offset_top(model.config.safe_area[1]),
        );

        let vm = Self::final_layout(model);
        let center = vm.view_box.center();
        let line_position = Line::new(0.0, vm.view_box.width(), center.y, center.y);
        // let buttons_position = model.tuning.as_ref().map_or_else(
        //     || Self::pairs_from_buttons(model),
        //     |v| {
        //         let mut init = Self::pairs_from_buttons(model);
        //         let full_h = vm.view_box.height() / 2.0;
        //         let pairs = Into::<Vec<f32>>::into(v.clone());
        //         init.iter_mut().enumerate().for_each(|(i, rect)| {
        //             *rect = rect.offset_top(full_h * pairs.get(i).cloned().unwrap_or(0.0) as f64);
        //         });
        //         init
        //     },
        // );

        let mut transition = keyframes![
            (
                IntroVM {
                    layout: instrument::Layout {
                        menu_position: init_menu_position.clone(),
                        ..vm.layout.clone()
                    },
                    ..vm.clone()
                },
                0.0
            ),
            (
                IntroVM {
                    menu_flip: 180.0,
                    layout: instrument::Layout {
                        menu_position: menu_button_position.clone(),
                        tracks: vec![],
                        ..vm.layout.clone()
                    },
                    ..vm.clone()
                },
                0.2,
                EaseOut
            ),
            (
                IntroVM {
                    menu_flip: 180.0,
                    layout: instrument::Layout {
                        menu_position: menu_button_position.clone(),
                        tracks: vec![],
                        inbound: line_position,
                        outbound: line_position,
                        ..vm.layout.clone()
                    },
                    ..vm.clone()
                },
                1.0,//0.35,
                EaseOut
            )
        ];

        if reverse {
            transition.reverse();
        }

        Self::TunerIntro(transition)
    }

    pub fn loading_intro(model: &Model) -> Self {
        let init_vm = IntroVM::default();
        let vb_target = Rect::size(model.config.width, model.config.height);
        let final_vm = Self::final_layout(model);

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

        let target_menu_position = Self::central_menu_position(model);

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
                    layout: dummy_layout.clone(),
                    button_size: target_button_size,
                    intro_opacity: 0.0,
                    ..init_vm.clone()
                },
                0.25,
                EaseOut
            ),
            (
                IntroVM {
                    layout: intermediate_layout.clone(),
                    ..final_vm.clone()
                },
                0.5,
                EaseOut
            ),
            (
                IntroVM {
                    layout: instrument::Layout {
                        tracks: tracks_intermediate,
                        ..intermediate_layout.clone()
                    },
                    ..final_vm.clone()
                },
                0.65,
                EaseOut
            ),
            (
                IntroVM {
                    layout: instrument::Layout {
                        menu_position: off_screen_menu_position.clone(),
                        ..final_vm.layout.clone()
                    },
                    ..final_vm.clone()
                },
                0.75,
                EaseIn
            ),
            (
                IntroVM {
                    intro_opacity: 0.0,
                    layout: instrument::Layout {
                        menu_position: target_menu_position.clone(),
                        ..final_vm.layout.clone()
                    },
                    ..final_vm.clone()
                },
                1.0,
                EaseOut
            )
        ];

        Self::LoadingIntro(animation)
    }
}
