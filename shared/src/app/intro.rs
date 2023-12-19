use crux_core::render::Render;
use crux_core::App;
use crux_macros::Effect;
use keyframe_derive::CanTween;
use mint::{Point2, Point3};
use serde::{Deserialize, Serialize};

use animation::IntroAnimation;

use crate::animate::Animate;
use crate::{geometry::Rect, instrument, Activity, Navigate};

mod animation;

#[derive(Default)]
pub struct Intro;

#[derive(Default)]
pub struct Model {
    pub layout: instrument::Layout,
    pub config: instrument::Config,
    pub sequence: Option<IntroAnimation>,
    pub reduced_motion: bool,
    pub transition_to: Option<Activity>,
    pub current_activity: Activity,
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
    pub menu_flip: f64,
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
            menu_flip: 0.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum IntroEV {
    SetInstrumentTarget(Box<instrument::Layout>, Box<instrument::Config>),
    SetReducedMotion(bool),
    TsNext(f64),
    Menu(Activity),
    Start,
}

impl Eq for IntroEV {}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Intro")]
pub struct IntroCapabilities {
    pub render: Render<IntroEV>,
    pub navigate: Navigate<IntroEV>,
    pub animate: Animate<IntroEV>,
}

impl App for Intro {
    type Event = IntroEV;

    type Model = Model;

    type ViewModel = IntroVM;

    type Capabilities = IntroCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            IntroEV::Start => caps.animate.start(IntroEV::TsNext),
            IntroEV::SetInstrumentTarget(layout, config) => {
                model.layout = *layout;
                model.config = *config;
                if let Some(mut current) = model.sequence.take() {
                    current.update_to_match(model, model.current_activity);
                    _ = model.sequence.insert(current);
                } else {
                    model.sequence = Some(IntroAnimation::new(model, model.current_activity))
                }
                caps.render.render();
            }
            IntroEV::SetReducedMotion(reduced_motion) => {
                model.reduced_motion = reduced_motion;
                caps.render.render();
            }
            IntroEV::TsNext(ts) => {
                if let Some(seq) = model.sequence.as_mut() {
                    let ended = seq.tick(ts, model.reduced_motion);
                    if ended {
                        if let Some(to) = model.transition_to.take() {
                            caps.navigate.to(to);
                            log::info!("navigating");
                        }
                        caps.animate.stop();
                        log::info!("{:?} animation ended", seq.animation);
                    }
                    caps.render.render();
                } else {
                    panic!("animation started without sequence");
                }
            }
            IntroEV::Menu(next_activity) => {
                model.sequence = Some(IntroAnimation::new(model, next_activity));
                match next_activity {
                    Activity::Intro => {
                        caps.navigate.to(Activity::Intro);
                    }
                    _ => {
                        _ = model.transition_to.insert(next_activity);
                        caps.animate.start(IntroEV::TsNext);
                    }
                }
                log::info!("scheduled transition to {next_activity:?}");
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
