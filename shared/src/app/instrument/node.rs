use super::keyboard::{Button, Track};
use hecs::{Entity, World};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Copy)]

pub struct Node {
    pub freq: (f32, f32),
    pub f_n: usize,
    pub pan: i8,
}

impl Eq for Node {}

impl Node {
    pub fn spawn(
        world: &mut World,
        freq: (f32, f32),
        f_n: usize,
        pan: i8,
    ) -> Entity {
        world.spawn((Self {
            freq,
            f_n,
            pan
        },))
    }
}
