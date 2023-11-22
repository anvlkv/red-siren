use keyframe_derive::CanTween;
use mint::{Vector2, Point2};
use serde::{Serialize, Deserialize};

use super::rect::Rect;

#[derive(Serialize, Deserialize, Clone, PartialEq, Copy, CanTween, Debug)]
pub struct Line {
    line: Vector2<Point2<f64>>,
}

impl Eq for Line {}

impl Default for Line {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

impl Line {
    pub fn new(x: f64, x1: f64, y: f64, y1: f64) -> Self {
        Self {
            line: Vector2 {
                x: Point2 { x, y },
                y: Point2 { x: x1, y: y1 },
            },
        }
    }

    pub fn vertical(len: f64) -> Self {
        Self::new(0.0, 0.0, 0.0, len,)
    }

    pub fn horizontal(len: f64) -> Self {
        Self::new(0.0, len, 0.0, 0.0)
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.line.x.x, self.line.y.x, self.line.x.y, self.line.y.y)
    }

    pub fn p0(&self) -> Point2<f64> {
        self.line.x
    }

    pub fn p1(&self) -> Point2<f64> {
        self.line.y
    }

    pub fn width(&self) -> f64 {
        self.line.y.x - self.line.x.x
    }

    pub fn height(&self) -> f64 {
        self.line.y.y - self.line.x.y
    }
}