use super::{
    keyboard::{Button, ButtonGroup, Keyboard, Track},
    string::{InboundString, OutboundString},
};
use crate::geometry::{line::Line, rect::Rect};
use anyhow::Result;
use hecs::{Bundle, Entity, World};
use keyframe::CanTween;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
pub struct Layout {
    pub inbound: Line,
    pub outbound: Line,
    pub buttons: Vec<Rect>,
    pub tracks: Vec<Rect>,
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

        Self {
            outbound,
            inbound,
            buttons,
            tracks,
        }
    }
}

impl Layout {
    pub fn new(world: &World, root: &Entity) -> Result<Self> {
        let root = world.get::<&LayoutRoot>(*root)?;
        let (inbound, keyboard, outbound) = (
            world.get::<&InboundString>(root.inbound())?,
            world.get::<&Keyboard>(root.keyboard())?,
            world.get::<&OutboundString>(root.outbound())?,
        );

        let (buttons, tracks) = keyboard
            .groups
            .iter()
            .try_fold(Vec::<(Rect, Rect)>::new(), |mut acc, g| -> Result<_> {
                let group = world.get::<&ButtonGroup>(*g)?;
                for b in &group.buttons {
                    let button = world.get::<&Button>(*b)?;
                    let track = world.get::<&Track>(button.track)?;
                    acc.push((button.rect, track.rect));
                }
                Ok(acc)
            })?
            .into_iter()
            .unzip();

        Ok(Self {
            inbound: inbound.line,
            outbound: outbound.line,
            buttons,
            tracks,
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
        .into_iter()
        .map(|i| {
            let (from_b, to_b) = (from.iter().nth(i), to.iter().nth(i));
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
