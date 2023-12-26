use hecs::{Entity, World};
use serde::{Deserialize, Serialize};

use super::keyboard::{Button, Track};

#[derive(Serialize, Deserialize, Clone, PartialEq, Copy, Debug)]

pub struct Node {
    pub freq: (f32, f32),
    pub f_n: usize,
    pub pan: i8,
}

impl Eq for Node {}

impl Node {
    pub fn spawn(world: &mut World, freq: (f32, f32), f_n: usize, pan: i8) -> Entity {
        log::debug!("node pan: {pan}");
        world.spawn((Self { freq, f_n, pan },))
    }
}

pub fn spawn_all_nodes(world: &mut World) -> Vec<Entity> {
    let mut nodes = world
        .query::<&Button>()
        .iter()
        .map(|(_, b)| {
            let mut query = world.query_one::<&Track>(b.track).unwrap();
            let track = query.get().unwrap();
            (track.freq, b.f_n, if track.left_hand { -1 } else { 1 })
        })
        .collect::<Vec<_>>();

    nodes.sort_by(|a, b| a.1.cmp(&b.1));
    nodes.reverse();

    nodes
        .into_iter()
        .map(|(freq, f_n, pan)| Node::spawn(world, freq, f_n, pan))
        .collect::<Vec<_>>()
}
