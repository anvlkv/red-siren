use anyhow::Result;
use hecs::{Bundle, Entity, World};
use keyframe::CanTween;
use serde::{Deserialize, Serialize};

use crate::geometry::{line::Line, rect::Rect};

use super::{
    keyboard::{Button, ButtonGroup, Keyboard, Track},
    string::{InboundString, OutboundString},
    Config,
};

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
pub struct Layout {
    pub inbound: Line,
    pub outbound: Line,
    pub buttons: Vec<Rect>,
    pub tracks: Vec<Rect>,
    pub menu_position: MenuPosition,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
pub enum MenuPosition {
    TopLeft(Rect),
    TopRight(Rect),
    BottomLeft(Rect),
    Center(Rect),
}

impl Default for MenuPosition {
    fn default() -> Self {
        Self::Center(Rect::default())
    }
}

impl MenuPosition {
    pub fn rect(&self) -> &Rect {
        match self {
            MenuPosition::Center(r)
            | MenuPosition::TopLeft(r)
            | MenuPosition::TopRight(r)
            | MenuPosition::BottomLeft(r) => r,
        }
    }
}

impl CanTween for MenuPosition {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let r1 = *from.rect();

        match to {
            MenuPosition::TopLeft(r) => MenuPosition::TopLeft(CanTween::ease(r1, r, time)),
            MenuPosition::Center(r) => MenuPosition::Center(CanTween::ease(r1, r, time)),
            MenuPosition::TopRight(r) => MenuPosition::TopRight(CanTween::ease(r1, r, time)),
            MenuPosition::BottomLeft(r) => MenuPosition::BottomLeft(CanTween::ease(r1, r, time)),
        }
    }
}

#[derive(Bundle)]
pub struct LayoutRoot {
    entities: (Entity, Entity, Entity),
}

impl LayoutRoot {
    pub fn spawn(world: &mut World, inbound: Entity, outbound: Entity, keyboard: Entity) -> Entity {
        world.spawn((Self {
            entities: (inbound, keyboard, outbound),
        },))
    }

    pub fn inbound(&self) -> Entity {
        self.entities.0
    }
    pub fn outbound(&self) -> Entity {
        self.entities.2
    }
    pub fn keyboard(&self) -> Entity {
        self.entities.1
    }
}

impl CanTween for Layout {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let outbound = CanTween::ease(from.outbound, to.outbound, time);
        let inbound = CanTween::ease(from.inbound, to.inbound, time);

        let buttons = ease_vec(from.buttons, to.buttons, time);
        let tracks = ease_vec(from.tracks, to.tracks, time);
        let menu_position = CanTween::ease(from.menu_position, to.menu_position, time);

        Self {
            outbound,
            inbound,
            buttons,
            tracks,
            menu_position,
        }
    }
}

impl Layout {
    pub fn new(world: &World, root: &Entity, config: &Config) -> Result<Self> {
        let root = world.get::<&LayoutRoot>(*root)?;
        let (inbound, keyboard, outbound) = (
            world.get::<&InboundString>(root.inbound())?,
            world.get::<&Keyboard>(root.keyboard())?,
            world.get::<&OutboundString>(root.outbound())?,
        );

        let (buttons, tracks): (Vec<Rect>, Vec<Rect>) = keyboard
            .groups
            .iter()
            .enumerate()
            .try_fold(
                Vec::<(Rect, Rect)>::new(),
                |mut acc, (_i, g)| -> Result<_> {
                    let group = world.get::<&ButtonGroup>(*g)?;
                    for b in &group.buttons {
                        let button = world.get::<&Button>(*b)?;
                        let track = world.get::<&Track>(button.track)?;
                        acc.push((button.rect, track.rect));
                    }
                    Ok(acc)
                },
            )?
            .into_iter()
            .unzip();

        log::debug!("tracks {tracks:#?}");

        let menu_position = tracks
            .first()
            .map(|t| {
                if config.portrait {
                    if t.top_left().x >= config.breadth {
                        MenuPosition::TopLeft(Rect::new(
                            config.safe_area[0],
                            config.breadth,
                            config.safe_area[1],
                            config.breadth,
                        ))
                    } else {
                        MenuPosition::TopRight(Rect::new(
                            config.width - config.breadth,
                            config.width - config.safe_area[2],
                            config.safe_area[1],
                            config.breadth,
                        ))
                    }
                } else if t.top_left().y < config.breadth {
                    MenuPosition::BottomLeft(Rect::new(
                        config.safe_area[0],
                        config.breadth,
                        config.height - config.breadth,
                        config.height - config.safe_area[3],
                    ))
                } else {
                    MenuPosition::TopLeft(Rect::new(
                        config.safe_area[0],
                        config.breadth,
                        config.safe_area[1],
                        config.breadth,
                    ))
                }
            })
            .unwrap_or_default();

        Ok(Self {
            inbound: inbound.line,
            outbound: outbound.line,
            buttons,
            tracks,
            menu_position,
        })
    }

    pub fn dummy(breadth: f64, length: f64, button_size: f64) -> Self {
        let inbound = Line::new(0.0, length, 0.0, 1.0);
        let outbound = Line::new(0.0, length, breadth, breadth + 1.0);

        let button = Rect::new(0.0, button_size, 0.0, button_size);

        Self {
            inbound,
            outbound,
            buttons: vec![button],
            ..Default::default()
        }
    }
}

fn ease_vec(from: Vec<Rect>, to: Vec<Rect>, time: impl keyframe::num_traits::Float) -> Vec<Rect> {
    let len = CanTween::ease(from.len() as f64, to.len() as f64, time).round() as usize;
    (0..len)
        .map(|i| {
            let (from_b, to_b) = (from.get(i), to.get(i));
            match (from_b, to_b) {
                (Some(from), Some(to)) => CanTween::ease(*from, *to, time),
                (Some(from), None) => {
                    let c = from.center();
                    let z_rect = Rect::new(c.x, c.x, c.y, c.y);
                    CanTween::ease(*from, z_rect, time)
                }
                (None, Some(to)) => {
                    let c = to.center();
                    let z_rect = Rect::new(c.x, c.x, c.y, c.y);
                    CanTween::ease(z_rect, *to, time)
                }
                (None, None) => unreachable!(),
            }
        })
        .collect()
}
