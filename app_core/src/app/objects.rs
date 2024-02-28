use ecolor::Rgba;
use euclid::default::{Box2D, Point2D, Size2D};
use hecs::{Bundle, Entity};
use keyframe::CanTween;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Objects(pub Vec<(Entity, Object)>);

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
            tween.0.push((e, CanTween::ease(from, to, time)))
        }
        tween
    }
}

#[derive(Bundle, Clone, Serialize, Deserialize, Builder)]
pub struct Object {
    #[builder(default = "uuid::Uuid::new_v4()")]
    pub id: Uuid,
    pub shape: Shapes,
    pub fill: Option<Rgba>,
    pub stroke: Option<Stroke>,
    pub a11y_label: Option<String>,
    pub view_label: Option<Text>,
}

impl CanTween for Object {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let id = from.id;
        assert_eq!(id, to.id, "same object id");
        let shape = CanTween::ease(from.shape, to.shape, time);
        let fill = ease_color_option(from.fill, to.fill, time);
        let stroke = if from.stroke.is_none() && to.stroke.is_none() {
            None
        } else {
            Some(CanTween::ease(
                from.stroke
                    .clone()
                    .unwrap_or(Stroke::zero(to.stroke.as_ref().map(|s| s.color))),
                to.stroke
                    .clone()
                    .unwrap_or(Stroke::zero(from.stroke.as_ref().map(|s| s.color))),
                time,
            ))
        };
        let a11y_label = to.a11y_label;
        let view_label = to.view_label;
        Self {
            id,
            shape,
            fill,
            stroke,
            a11y_label,
            view_label,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Rgba,
    pub width: f64,
}

impl Stroke {
    pub fn zero(color: Option<Rgba>) -> Self {
        Self {
            color: color.unwrap_or(Rgba::TRANSPARENT),
            width: 0.0,
        }
    }
}

impl CanTween for Stroke {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let width = CanTween::ease(from.width, to.width, time);
        let color = ease_color_option(Some(from.color), Some(to.color), time).unwrap();
        Self { width, color }
    }
}

fn ease_color_option(
    from: Option<Rgba>,
    to: Option<Rgba>,
    time: impl keyframe::num_traits::Float,
) -> Option<Rgba> {
    match (from, to) {
        (None, None) => None,
        (Some(c1), None) => {
            let a = CanTween::ease(c1.a(), 0.0, time);
            Some(Rgba::from_rgba_unmultiplied(c1.r(), c1.g(), c1.b(), a))
        }
        (None, Some(c1)) => {
            let a = CanTween::ease(0.0, c1.a(), time);
            Some(Rgba::from_rgba_unmultiplied(c1.r(), c1.g(), c1.b(), a))
        }
        (Some(c1), Some(c2)) => {
            let r = CanTween::ease(c1.r(), c2.r(), time);
            let g = CanTween::ease(c1.g(), c2.g(), time);
            let b = CanTween::ease(c1.b(), c2.b(), time);
            let a = CanTween::ease(c1.a(), c2.a(), time);
            Some(Rgba::from_rgba_unmultiplied(r, g, b, a))
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Shapes {
    Path(Vec<Point2D<f64>>),
    Circle(Box2D<f64>),
    RoundedRect(Box2D<f64>, Size2D<f64>),
}

impl Shapes {
    pub fn containing_rect(&self) -> Box2D<f64> {
        match self {
            Self::Path(points) => {
                let x_min = points
                    .iter()
                    .min_by(|p1, p2| {
                        PartialOrd::partial_cmp(&p1.x, &p2.x).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|p| p.x)
                    .unwrap_or_default();

                let x_max = points
                    .iter()
                    .max_by(|p1, p2| {
                        PartialOrd::partial_cmp(&p1.x, &p2.x).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|p| p.x)
                    .unwrap_or_default();

                let y_min = points
                    .iter()
                    .min_by(|p1, p2| {
                        PartialOrd::partial_cmp(&p1.y, &p2.y).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|p| p.y)
                    .unwrap_or_default();

                let y_max = points
                    .iter()
                    .min_by(|p1, p2| {
                        PartialOrd::partial_cmp(&p1.y, &p2.y).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|p| p.y)
                    .unwrap_or_default();

                Box2D::new(Point2D::new(x_min, y_min), Point2D::new(x_max, y_max))
            }
            Self::Circle(rect) => rect.clone(),
            Self::RoundedRect(rect, _) => rect.clone(),
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

impl CanTween for Shapes {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        match (from, to) {
            (Shapes::Path(p1), Shapes::Path(p2)) => {
                let mut path = vec![];
                let count = CanTween::ease(p1.len() as f32, p2.len() as f32, time).round() as usize;
                for i in 0..count {
                    let from_x = p1.get(i).or_else(|| p1.last()).map(|p| p.x).unwrap_or(0.0);
                    let from_y = p1.get(i).or_else(|| p1.last()).map(|p| p.y).unwrap_or(0.0);
                    let to_x = p2.get(i).or_else(|| p2.last()).map(|p| p.x).unwrap_or(0.0);
                    let to_y = p2.get(i).or_else(|| p2.last()).map(|p| p.y).unwrap_or(0.0);
                    path.push(Point2D::new(
                        CanTween::ease(from_x, to_x, time),
                        CanTween::ease(from_y, to_y, time),
                    ))
                }
                Shapes::Path(path)
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

#[derive(Clone, Serialize, Deserialize)]
pub enum Alignment {
    Leading,
    Trailing,
    Center,
}
