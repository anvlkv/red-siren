use std::num::NonZero;

use mint::Vector2;

use crate::{orientation::LayoutOrientation, safe_area::SafeArea, Line};

pub struct Layout {
    /// Total screen estate available to layout the tuner
    pub space: Vector2<f32>,
    /// Whether `Horizontal` or `Vertical` layout is used,
    /// determines the main and auxilary axes of the tuner layout
    pub orientation: LayoutOrientation,
    /// Minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// Start and end postions of **both** lines of tuner's FFT
    pub line_position: Line,
    /// Radius of each sensor
    pub sensor_radius: f32,
    /// Max split of each sensor
    pub sensor_max_split: Vector2<f32>,
    /// Number of groups
    pub num_groups: NonZero<u8>,
    /// Number of keys and bands in each group
    pub num_keys_per_group: NonZero<u8>,
}

