use hecs::{Bundle, Entity, World};
use mint::Point2;

use crate::geometry::Line;

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
    pub data: Vec<Point2<f64>>,
}

impl OutboundString {
    pub fn spawn(world: &mut World, config: &Config) -> Entity {
        world.spawn((OutboundString {
            line: string_line(config, 1.0),
            data: vec![],
        },))
    }

    pub fn update_data(&mut self, data: Vec<f32>, config: &Config) {
        let l_step = config.length / data.len() as f64;
        let b_step = config.breadth / 2.25;
        let b_base = if config.portrait {
            self.line.p0().x
        } else {
            self.line.p0().y
        };

        self.data = data.into_iter().enumerate().map(|(i, val)| {
            let val = val * 512.0;
            if config.portrait {
                Point2 {
                    x: b_base + b_step * val as f64,
                    y: i as f64 * l_step,
                }
            } else {
                Point2 {
                    y: b_base + b_step * val as f64,
                    x: i as f64 * l_step,
                }
            }
        }).collect();
    }
}

fn string_line(config: &Config, at: f64) -> Line {
    if config.portrait {
        let main = (config.width - config.breadth) / at;
        Line::new(
            main,
            main,
            0.0,
            config.length + config.safe_area[3] + config.safe_area[1] + config.whitespace * 2.0,
        )
    } else {
        let main = (config.height - config.breadth) / at;
        Line::new(
            0.0,
            config.length + config.safe_area[2] + config.safe_area[0] + config.whitespace * 2.0,
            main,
            main,
        )
    }
}
