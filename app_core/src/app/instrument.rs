use std::ops::Deref;

use anyhow::{anyhow, Result};
use au_core::Node;
use hecs::{Entity, World};

use super::{config::Config, layout::Layout};

#[derive(Clone, Default)]
pub struct Instrument {
    pub nodes: Vec<Entity>,
}

impl Instrument {
    pub fn new(config: &Config, world: &mut World, layout: &Layout) -> Result<Self> {
        let mut nodes = vec![];
        let stereo = config.groups >= 2;
        for gx in 0..config.groups {
            let left_hand = (gx + 1) % 2 == 0;

            for bx in 0..config.buttons_group {
                let idx = gx * config.buttons_group + bx;

                let button = layout
                    .buttons
                    .get(idx)
                    .ok_or(anyhow!("no button for idx {idx}"))?;

                let pan = if stereo {
                    if left_hand {
                        -0.95
                    } else {
                        0.95
                    }
                } else {
                    0_f32
                };

                let node_obj = Self::make_node(config, idx, *button, pan)?;

                let node = world.spawn((node_obj,));

                nodes.push(node);
            }
        }

        Ok(Self { nodes })
    }

    fn make_node(config: &Config, idx: usize, button: Entity, pan: f32) -> Result<Node> {
        let f_n = config.n_buttons - idx;
        let freq = config.f0 * (f_n * 2) as f32 - config.f0;
        let max_freq = freq + config.f0;

        let node_data = au_core::NodeDataBuilder::default()
            .button(button)
            .f_base(freq)
            .f_emit((freq, max_freq))
            .f_sense(((freq, max_freq), (0.0, 1.0)))
            .pan(pan)
            .build()?;

        Ok(node_data.into())
    }

    pub fn get_nodes(&self, world: &World) -> Vec<Node> {
        self.nodes
            .iter()
            .filter_map(|e| world.get::<&Node>(*e).ok().map(|n| n.deref().clone()))
            .collect()
    }
}
