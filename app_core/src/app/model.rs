use hecs::{World, Entity};
use std::sync::{Arc, Mutex};
use euclid::default::{Box2D, SideOffsets2D};
use keyframe::AnimationSequence;

use super::objects::Objects;

#[derive(Default)]
pub struct Model {
  pub world: Arc<Mutex<World>>,
  pub activity: super::Activity,
  pub view_box: Box2D<f64>,
  pub safe_box: Box2D<f64>,
  pub safe_area: SideOffsets2D<f64>,
  pub reduced_motion: bool,
  pub intro_opacity: Option<AnimationSequence<f64>>,
  pub view_objects_animation: Option<AnimationSequence<Objects>>,
  pub running_animation: Option<(f64, f64)>,
  pub buttons: Vec<Entity>,
  pub strings: Vec<Entity>,
  pub tracks: Vec<Entity>,
  pub nodes: Vec<Entity>,
}