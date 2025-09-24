use std::num::NonZero;

use mint::{Point2, Vector2};

use crate::{orientation::LayoutOrientation, safe_area::SafeArea};

pub struct Layout {
    /// total screen estate available to layout the tuner
    pub space: Vector2<f32>,
    /// whether `Horizontal` or `Vertical` layout is used,
    /// determines the main and auxilary axes of the tuner layout
    pub orientation: LayoutOrientation,
    /// minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// start and end postions of **current** line of tuner's FFT
    pub current_line_position: (Point2<f32>, Point2<f32>),
    /// start and end postions of **max** line of tuner's FFT
    pub max_line_position: (Point2<f32>, Point2<f32>),
    /// radius of each sensor
    pub sensor_radius: f32,
    /// max split of each sensor
    pub sensor_max_split: Vector2<f32>,
    /// number of groups
    pub num_groups: NonZero<u8>,
}
