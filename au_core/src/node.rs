use fundsp::hacker32::*;
use hecs::{Bundle, Entity};

#[derive(Bundle, Clone)]
pub struct Node {
    pub f_base: Shared<f32>,
    pub f_emit: (Shared<f32>, Shared<f32>),
    pub f_sense: ((Shared<f32>, Shared<f32>), (Shared<f32>, Shared<f32>)),
    pub control: Shared<f32>,
    pub button: Entity,
    pub pan: Shared<f32>,
}

impl Node {
  pub fn new(button: Entity) -> Node {
    Node {
      button,
      f_base: shared(0.0),
      f_emit: (shared(0.0), shared(0.0)),
      f_sense: ((shared(0.0), shared(0.0)), (shared(0.0), shared(0.0))),
      control: shared(0.0),
      pan: shared(0.0),
    }
  }

  pub fn dummy() -> Self {
    Self::new(Entity::DANGLING)
  }
}