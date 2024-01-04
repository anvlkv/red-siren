use keyframe_derive::CanTween;
use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Copy, CanTween, Debug)]
pub struct Rect {
    rect: Vector2<Point2<f64>>,
}

impl Eq for Rect {}

impl Default for Rect {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

impl Rect {
    pub fn new(left: f64, right: f64, top: f64, bottom: f64) -> Self {
        Self {
            rect: Vector2 {
                x: Point2 {
                    x: left.min(right),
                    y: top.min(bottom),
                },
                y: Point2 {
                    x: right.max(left),
                    y: bottom.max(top),
                },
            },
        }
    }

    pub fn size(width: f64, height: f64) -> Self {
        Self::new(0.0, width, 0.0, height)
    }

    pub fn components(&self) -> (f64, f64, f64, f64) {
        (self.rect.x.x, self.rect.y.x, self.rect.x.y, self.rect.y.y)
    }

    pub fn center(&self) -> Point2<f64> {
        let d_x = self.width() / 2.0;
        let d_y = self.height() / 2.0;

        Point2 {
            x: self.rect.x.x + d_x,
            y: self.rect.x.y + d_y,
        }
    }

    pub fn top_left(&self) -> Point2<f64> {
        Point2 {
            x: self.rect.x.x,
            y: self.rect.x.y,
        }
    }

    pub fn bottom_right(&self) -> Point2<f64> {
        Point2 {
            x: self.rect.y.x,
            y: self.rect.y.y,
        }
    }

    pub fn top_right(&self) -> Point2<f64> {
        Point2 {
            x: self.rect.y.x,
            y: self.rect.x.y,
        }
    }

    pub fn bottom_left(&self) -> Point2<f64> {
        Point2 {
            x: self.rect.x.x,
            y: self.rect.y.y,
        }
    }

    pub fn width(&self) -> f64 {
        self.rect.y.x - self.rect.x.x
    }

    pub fn height(&self) -> f64 {
        self.rect.y.y - self.rect.x.y
    }

    pub fn offset_left(&self, by: f64) -> Self {
        Self::new(
            self.rect.x.x - by,
            self.rect.y.x,
            self.rect.x.y,
            self.rect.y.y,
        )
    }

    pub fn offset_top(&self, by: f64) -> Self {
        Self::new(
            self.rect.x.x,
            self.rect.y.x,
            self.rect.x.y - by,
            self.rect.y.y,
        )
    }

    pub fn offset_right(&self, by: f64) -> Self {
        Self::new(
            self.rect.x.x,
            self.rect.y.x + by,
            self.rect.x.y,
            self.rect.y.y,
        )
    }

    pub fn offset_bottom(&self, by: f64) -> Self {
        Self::new(
            self.rect.x.x,
            self.rect.y.x,
            self.rect.x.y,
            self.rect.y.y + by,
        )
    }

    pub fn offset_top_and_bottom(&self, top: f64, bottom: f64) -> Self {
        Self::new(
            self.rect.x.x,
            self.rect.y.x,
            self.rect.x.y - top,
            self.rect.y.y + bottom,
        )
    }

    pub fn offset_left_and_right(&self, left: f64, right: f64) -> Self {
        Self::new(
            self.rect.x.x - left,
            self.rect.y.x + right,
            self.rect.x.y,
            self.rect.y.y,
        )
    }
}
