use super::Animate;
use crux_core::render::Render;
pub use crux_core::App;
use crux_macros::Effect;
use euclid::default::{Box2D, Point2D, SideOffsets2D};
use keyframe::{functions::EaseOut, keyframes, AnimationSequence};
use serde::{Deserialize, Serialize};

const INTRO_DURATION: f64 = 2750.0;
const TRANSITION_DURATION: f64 = 750.0;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct VisualVM {
    pub width: f64,
    pub height: f64,
    pub intro_opacity: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum VisualEV {
    Resize(f64, f64),
    SafeAreaResize(f64, f64, f64, f64),
    AnimateEntrance,
    AnimateEntranceTS(f64),
    SetReducedMotion(bool),
    SetDensity(f64),
}

#[derive(Default)]
pub struct Visual {}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Visual")]
pub struct VisualCapabilities {
    pub render: Render<VisualEV>,
    pub animate: Animate<VisualEV>,
}

impl App for Visual {
    type Event = VisualEV;
    type Model = super::model::Model;
    type ViewModel = VisualVM;
    type Capabilities = VisualCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            VisualEV::AnimateEntrance => {
                model.intro_opacity = Some(keyframes![(0.0, 1.0, EaseOut), (1.0, 0.0)]);
                // model.view_objects_animation = 
                caps.animate
                    .start(VisualEV::AnimateEntranceTS, "intro animation".to_string())
            }
            VisualEV::AnimateEntranceTS(ts) => {
                let (start, now) = model.running_animation.get_or_insert((ts, ts));
                *now = ts;

                let advance_duration = (ts - *start) / INTRO_DURATION;

                let seq = model.intro_opacity.as_mut().unwrap();

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

                caps.render.render();

                if seq.finished() {
                    caps.animate.stop();
                }
            }
            VisualEV::SetReducedMotion(reduce) => {
                model.reduced_motion = reduce;
                caps.render.render();
            }
            VisualEV::SetDensity(density) => {
                self.set_density(density, model);
                caps.render.render();
            }
            VisualEV::Resize(width, height) => {
                self.resize(width, height, model);
                caps.render.render();
            }
            VisualEV::SafeAreaResize(left, top, right, bottom) => {
                self.safe_area(left, top, right, bottom, model);
                caps.render.render();
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        VisualVM {
            width: model.view_box.width(),
            height: model.view_box.height(),
            intro_opacity: model.intro_opacity.as_ref().map(|s| s.now()).unwrap_or(1.0),
        }
    }
}

impl Visual {
    pub fn safe_area(
        &self,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        model: &mut super::model::Model,
    ) {
        model.safe_area = SideOffsets2D::new(top, right, bottom, left);
        self.resize(model.view_box.width(), model.view_box.height(), model);
    }

    pub fn set_density(&self, density: f64, model: &mut super::model::Model) {
        model.density = density;
        self.resize(model.view_box.width(), model.view_box.height(), model);
    }
    pub fn resize(&self, width: f64, height: f64, model: &mut super::model::Model) {
        model.view_box = Box2D::new(Point2D::default(), Point2D::new(width, height));
        model.safe_box = model.view_box.inner_box(model.safe_area);
        // todo: handle density difference
    }
}
