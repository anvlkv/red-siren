use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};

use crate::{safe_area::SafeArea, Line};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutOrientation {
    Vertical,
    Horizontal,
}

impl LayoutOrientation {
    pub fn from_space(space: Vector2<f64>) -> Self {
        if space.x >= space.y {
            LayoutOrientation::Horizontal
        } else {
            LayoutOrientation::Vertical
        }
    }

    pub fn length(&self, space: Vector2<f64>) -> f64 {
        match self {
            LayoutOrientation::Horizontal => space.x,
            LayoutOrientation::Vertical => space.y,
        }
    }

    pub fn breadth(&self, space: Vector2<f64>) -> f64 {
        match self {
            LayoutOrientation::Horizontal => space.y,
            LayoutOrientation::Vertical => space.x,
        }
    }

    pub fn safe_length(&self, space: Vector2<f64>, safe_area: SafeArea) -> f64 {
        match self {
            LayoutOrientation::Horizontal => space.x - safe_area.left - safe_area.right,
            LayoutOrientation::Vertical => space.y - safe_area.top - safe_area.bottom,
        }
    }

    pub fn safe_breadth(&self, space: Vector2<f64>, safe_area: SafeArea) -> f64 {
        match self {
            LayoutOrientation::Horizontal => space.y - safe_area.top - safe_area.bottom,
            LayoutOrientation::Vertical => space.x - safe_area.left - safe_area.right,
        }
    }

    pub fn safe_length_start_point(&self, point: Point2<f64>, safe_area: SafeArea) -> Point2<f64> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x,
                y: point.y + safe_area.top,
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x + safe_area.left,
                y: point.y,
            },
        }
    }

    pub fn safe_length_end_point(&self, point: Point2<f64>, safe_area: SafeArea) -> Point2<f64> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x,
                y: point.y - safe_area.bottom,
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x - safe_area.right,
                y: point.y,
            },
        }
    }

    pub fn safe_breadth_start_point(&self, point: Point2<f64>, safe_area: SafeArea) -> Point2<f64> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x + safe_area.left,
                y: point.y,
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x,
                y: point.y + safe_area.top,
            },
        }
    }

    pub fn safe_breadth_end_point(&self, point: Point2<f64>, safe_area: SafeArea) -> Point2<f64> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x - safe_area.bottom,
                y: point.y,
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x,
                y: point.y - safe_area.right,
            },
        }
    }

    pub fn n_nth_along_dbe(
        &self,
        (start, end): Line,
        at: usize,
        num_divisions: usize,
    ) -> Point2<f64> {
        if num_divisions == 0 {
            return start;
        }
        let mut t = at as f64 / num_divisions as f64;
        if t > 1.0 {
            t = 1.0;
        }
        match self {
            LayoutOrientation::Horizontal => Point2 {
                x: start.x + (end.x - start.x) * t,
                y: start.y,
            },
            LayoutOrientation::Vertical => Point2 {
                x: start.x,
                y: start.y + (end.y - start.y) * t,
            },
        }
    }
}
