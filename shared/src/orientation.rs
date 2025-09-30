use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};

use crate::safe_area::SafeArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutOrientation {
    Vertical,
    Horizontal,
}

impl LayoutOrientation {
    pub fn from_space(space: Vector2<f32>) -> Self {
        if space.x >= space.y {
            LayoutOrientation::Horizontal
        } else {
            LayoutOrientation::Vertical
        }
    }

    pub fn length(&self, space: Vector2<f32>) -> f32 {
        match self {
            LayoutOrientation::Horizontal => space.x,
            LayoutOrientation::Vertical => space.y,
        }
    }

    pub fn breadth(&self, space: Vector2<f32>) -> f32 {
        match self {
            LayoutOrientation::Horizontal => space.y,
            LayoutOrientation::Vertical => space.x,
        }
    }

    pub fn safe_length(&self, space: Vector2<f32>, safe_area: SafeArea) -> f32 {
        match self {
            LayoutOrientation::Horizontal => space.x - safe_area[0] - safe_area[2],
            LayoutOrientation::Vertical => space.y - safe_area[0] - safe_area[2],
        }
    }

    pub fn safe_breadth(&self, space: Vector2<f32>, safe_area: SafeArea) -> f32 {
        match self {
            LayoutOrientation::Horizontal => space.y - safe_area[1] - safe_area[3],
            LayoutOrientation::Vertical => space.x - safe_area[1] - safe_area[3],
        }
    }

    pub fn safe_length_start_point(&self, point: Point2<f32>, safe_area: SafeArea) -> Point2<f32> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x,
                y: point.y + safe_area[0],
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x + safe_area[0],
                y: point.y,
            },
        }
    }

    pub fn safe_length_end_point(&self, point: Point2<f32>, safe_area: SafeArea) -> Point2<f32> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x,
                y: point.y - safe_area[2],
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x - safe_area[2],
                y: point.y,
            },
        }
    }

    pub fn safe_breadth_start_point(&self, point: Point2<f32>, safe_area: SafeArea) -> Point2<f32> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x + safe_area[1],
                y: point.y,
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x,
                y: point.y + safe_area[1],
            },
        }
    }

    pub fn safe_breadth_end_point(&self, point: Point2<f32>, safe_area: SafeArea) -> Point2<f32> {
        match self {
            LayoutOrientation::Vertical => Point2 {
                x: point.x - safe_area[3],
                y: point.y,
            },
            LayoutOrientation::Horizontal => Point2 {
                x: point.x,
                y: point.y - safe_area[3],
            },
        }
    }
}
