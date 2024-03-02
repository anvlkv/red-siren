use euclid::default::{Box2D, SideOffsets2D};
use hecs::{Entity, World};
use keyframe::AnimationSequence;
use std::sync::{Arc, Mutex};

use super::{config::Config, layout::Layout, objects::Objects};

#[derive(Default)]
pub struct Model {
    pub world: Arc<Mutex<World>>,
    pub activity: super::Activity,
    pub configs: Vec<Config>,
    pub current_config: usize,
    pub layout: Layout,
    pub objects: Objects,
    // visual
    pub view_box: Box2D<f64>,
    pub safe_box: Box2D<f64>,
    pub safe_area: SideOffsets2D<f64>,
    pub density: f64,
    pub reduced_motion: bool,
    pub dark_schema: bool,
    // animations
    pub intro_opacity: Option<AnimationSequence<f64>>,
    pub view_objects_animation: Option<AnimationSequence<Objects>>,
    pub running_animation: Option<(f64, f64)>,
}
