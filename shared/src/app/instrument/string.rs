use crate::geometry::Line;
use hecs::{Bundle, Entity, World};

use super::Config;

#[derive(Default, Bundle)]
pub struct InboundString {
    pub line: Line,
}

impl InboundString {
    pub fn spawn(world: &mut World, config: &Config) -> Entity {
        world.spawn((InboundString {
            line: if config.portrait {
                let main = (config.width - config.breadth) / 2.0;
                Line::new(main, main, 0.0, config.length)
            } else {
                let main = (config.height - config.breadth) / 2.0;
                Line::new(0.0, config.length, main, main)
            },
        },))
    }
}

#[derive(Default, Bundle)]
pub struct OutboundString {
    pub line: Line,
}

impl OutboundString {
    pub fn spawn(world: &mut World, config: &Config) -> Entity {
        world.spawn((OutboundString {
            line: if config.portrait {
                let main = config.width - config.breadth;
                Line::new(main, main, 0.0, config.length)
            } else {
                let main = config.height - config.breadth;
                Line::new(0.0, config.length, main, main)
            },
        },))
    }
}
