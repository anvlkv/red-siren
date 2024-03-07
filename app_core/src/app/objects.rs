use std::{
    hash::{Hash, Hasher},
    ops::Deref,
};

use crate::ObjectStyle;
use anyhow::Result;
use euclid::default::{Box2D, Point2D, Size2D};
use hecs::{Bundle, ComponentError, Entity, World};
use keyframe::CanTween;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{layout::Layout, Paint};

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Objects(pub Vec<((Entity, Entity), (Object, Paint))>);

impl Objects {
    pub fn make_paint(
        world: &mut World,
        e: &Entity,
        dark: bool,
        style: ObjectStyle,
    ) -> ((Entity, Entity), (Object, Paint)) {
        let paint_obj = Paint::new(*e, dark, style);
        let paint = world.spawn((paint_obj.clone(),));
        let obj = world.get::<&Object>(*e).unwrap();

        ((*e, paint), (obj.deref().clone(), paint_obj))
    }

    pub fn repaint(&mut self, world: &mut World, dark: bool) -> Result<()> {
        for ((_, e), _) in self.0.iter() {
            world.get::<&mut Paint>(*e)?.repaint(dark);
        }

        self.update_from_world(&world)?;

        Ok(())
    }

    pub fn painted_objects(&self) -> Vec<(Object, Paint)> {
        let (_, objects): (Vec<(Entity, Entity)>, Vec<(Object, Paint)>) =
            self.0.iter().cloned().unzip();

        objects
    }

    pub fn update_from_world(&mut self, world: &World) -> Result<()> {
        self.0.iter_mut().try_for_each(|((e, p), values)| {
            let obj = world.get::<&Object>(*e)?;
            let paint = world.get::<&Paint>(*p)?;
            *values = (obj.deref().clone(), paint.deref().clone());
            Ok::<(), ComponentError>(())
        })?;
        Ok(())
    }
}

impl CanTween for Objects {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        assert_eq!(
            from.0.len(),
            to.0.len(),
            "won't tween removals and additions"
        );
        assert!(
            from.0.iter().enumerate().all(|(i, (e, _))| to.0[i].0 == *e),
            "won't tween z order"
        );

        let mut tween = Self(vec![]);
        for ((e, from), (_, to)) in from.0.into_iter().zip(to.0) {
            tween.0.push((
                e,
                (
                    CanTween::ease(from.0, to.0, time),
                    CanTween::ease(from.1, to.1, time),
                ),
            ))
        }
        tween
    }
}

#[derive(Bundle, Clone, Serialize, Deserialize, Builder, Hash)]
pub struct Object {
    #[builder(default = "uuid::Uuid::new_v4()")]
    pub id: Uuid,
    pub shape: Shapes,
    #[builder(default = "None")]
    pub a11y_label: Option<String>,
    #[builder(default = "None")]
    pub view_label: Option<Text>,
}

impl CanTween for Object {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let id = from.id;
        assert_eq!(id, to.id, "same object id");
        let shape = CanTween::ease(from.shape, to.shape, time);

        let a11y_label = to.a11y_label;
        let view_label = to.view_label;
        Self {
            id,
            shape,
            a11y_label,
            view_label,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Text {
    pub text: String,
    pub base: Point2D<f64>,
    pub size: f64,
    pub alignment: Alignment,
}

impl Hash for Text {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        let bin = bincode::serialize(self).unwrap();
        bin.hash(state)
    }
}

#[derive(Clone, Serialize, Deserialize, Hash)]
pub enum Alignment {
    Leading,
    Trailing,
    Center,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Shapes {
    Path {
        path: Vec<Point2D<f64>>,
        p0: Point2D<f64>,
        p1: Point2D<f64>,
    },
    Circle(Box2D<f64>),
    RoundedRect(Box2D<f64>, Size2D<f64>),
}

impl Hash for Shapes {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        let bin = bincode::serialize(self).unwrap();
        bin.hash(state)
    }
}

impl Shapes {
    pub fn containing_rect(&self) -> Box2D<f64> {
        match self {
            Self::Path { p0, p1, .. } => {
                Box2D::new(Point2D::new(p0.x, p0.y), Point2D::new(p1.x, p1.y))
            }
            Self::Circle(rect) => rect.clone(),
            Self::RoundedRect(rect, _) => rect.clone(),
        }
    }
}

impl CanTween for Shapes {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        match (from, to) {
            (
                Shapes::Path {
                    path: path_from,
                    p0: p0_from,
                    p1: p1_from,
                },
                Shapes::Path {
                    path: path_to,
                    p0: p0_to,
                    p1: p1_to,
                },
            ) => {
                let mut path = vec![];
                let count = CanTween::ease(path_from.len() as f32, path_to.len() as f32, time)
                    .round() as usize;
                for i in 0..count {
                    let from_x = path_from.get(i).map(|p| p.x).unwrap_or(p1_from.x);
                    let from_y = path_from.get(i).map(|p| p.y).unwrap_or(p1_from.y);
                    let to_x = path_to.get(i).map(|p| p.x).unwrap_or(p1_to.x);
                    let to_y = path_to.get(i).map(|p| p.y).unwrap_or(p1_to.y);
                    path.push(Point2D::new(
                        CanTween::ease(from_x, to_x, time),
                        CanTween::ease(from_y, to_y, time),
                    ))
                }
                let p0 = Point2D::new(
                    CanTween::ease(p0_from.x, p0_to.x, time),
                    CanTween::ease(p0_from.y, p0_to.y, time),
                );
                let p1 = Point2D::new(
                    CanTween::ease(p1_from.x, p1_to.x, time),
                    CanTween::ease(p1_from.y, p1_to.y, time),
                );

                Shapes::Path { path, p0, p1 }
            }
            (Shapes::Circle(b1), Shapes::Circle(b2)) => {
                let max_x = CanTween::ease(b1.max.x, b2.max.x, time);
                let min_x = CanTween::ease(b1.min.x, b2.min.x, time);
                let max_y = CanTween::ease(b1.max.y, b2.max.y, time);
                let min_y = CanTween::ease(b1.min.y, b2.min.y, time);
                Shapes::Circle(Box2D::new(
                    Point2D::new(min_x, min_y),
                    Point2D::new(max_x, max_y),
                ))
            }
            (Shapes::RoundedRect(b1, r1), Shapes::RoundedRect(b2, r2)) => {
                let max_x = CanTween::ease(b1.max.x, b2.max.x, time);
                let min_x = CanTween::ease(b1.min.x, b2.min.x, time);
                let max_y = CanTween::ease(b1.max.y, b2.max.y, time);
                let min_y = CanTween::ease(b1.min.y, b2.min.y, time);
                let rx = CanTween::ease(r1.width, r2.width, time);
                let ry = CanTween::ease(r1.height, r2.height, time);
                Shapes::RoundedRect(
                    Box2D::new(Point2D::new(min_x, min_y), Point2D::new(max_x, max_y)),
                    Size2D::new(rx, ry),
                )
            }
            _ => panic!("variable shapes not supported"),
        }
    }
}
