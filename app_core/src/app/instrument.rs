use std::{collections::HashMap, ops::Deref};

use anyhow::{anyhow, Result};
use hecs::{Entity, World};
use shared::{NodeData, NodeDataBuilder};

use super::{config::Config, layout::Layout};

#[derive(Clone, Default)]
pub struct Instrument {
    pub nodes: Vec<Entity>,
    pub buttons_to_strings: HashMap<Entity, (Entity, Option<Entity>)>,
}

impl Instrument {
    pub fn new(config: &Config, world: &mut World, layout: &Layout) -> Result<Self> {
        let mut buttons_to_strings = HashMap::new();
        let mut nodes = vec![];
        let is_stereo = config.groups >= 2;

        let mut left_string_it = layout.left_strings.clone().into_iter();
        let mut right_string_it = layout.right_strings.clone().into_iter();

        for gx in 0..config.groups {
            let left_hand = (gx + 1) % 2 == 0;

            for bx in 0..config.buttons_group {
                let idx = gx * config.buttons_group + bx;

                let button = layout
                    .buttons
                    .get(idx)
                    .ok_or(anyhow!("no button for idx {idx}"))?;

                let pan = if is_stereo {
                    if left_hand {
                        -1.0
                    } else {
                        1.0
                    }
                } else {
                    0_f32
                };

                let string = if left_hand {
                    left_string_it.next()
                } else {
                    right_string_it.next()
                }
                .expect("string for node");

                let secondary_string = if !is_stereo {
                    if left_hand {
                        right_string_it.next()
                    } else {
                        left_string_it.next()
                    }
                } else {
                    None
                };

                _ = buttons_to_strings.insert(*button, (string, secondary_string));

                let node_obj = Self::make_node(config, idx, *button, pan)?;

                let node = world.spawn((node_obj,));

                nodes.push(node);
            }
        }

        Ok(Self {
            nodes,
            buttons_to_strings,
        })
    }

    fn make_node(config: &Config, idx: usize, button: Entity, pan: f32) -> Result<NodeData> {
        let f_n = config.n_buttons - idx;
        let freq = config.f0 * (f_n * 2) as f32 - config.f0;
        let max_freq = freq + config.f0;

        let node_data = NodeDataBuilder::default()
            .button(button)
            .f_base(config.f0)
            .f_emit((freq, max_freq))
            .f_sense(((freq, max_freq), (0.01, 1.0)))
            .pan(pan)
            .build()?;

        Ok(node_data)
    }

    pub fn get_nodes(&self, world: &World) -> Vec<NodeData> {
        self.nodes
            .iter()
            .filter_map(|e| world.get::<&NodeData>(*e).ok().map(|n| n.deref().clone()))
            .collect()
    }
}
