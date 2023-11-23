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
            line: string_line(config, 2.0),
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
            line: string_line(config, 1.0),
        },))
    }
}

fn string_line(config: &Config, at: f64) -> Line {
    if config.portrait {
        let main = (config.width - config.breadth) / at;
        Line::new(main, main, 0.0, config.length + config.safe_area[3] + config.safe_area[1])
    } else {
        let main = (config.height - config.breadth) / at;
        Line::new(0.0, config.length + config.safe_area[2] + config.safe_area[0], main, main)
    }
}