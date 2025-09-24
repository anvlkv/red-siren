use std::num::NonZero;

use mint::{Point2, Vector2};

use crate::{orientation::LayoutOrientation, safe_area::SafeArea};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// total screen estate available to layout the instrument
    pub space: Vector2<f32>,
    /// whether `Horizontal` or `Vertical` layout is used,
    /// determines the main and auxilary axes of the instrument layout
    pub orientation: LayoutOrientation,
    /// start and end postions of **left** channel string
    pub left_string_position: (Point2<f32>, Point2<f32>),
    /// start and end postions of **right** channel string
    pub right_string_position: (Point2<f32>, Point2<f32>),
    /// radius of each key
    pub key_radius: f32,
    /// length of the `track` of key's band
    pub key_band_length: f32,
    /// breadth of the `track` of key's band
    pub key_band_breadth: f32,
    /// minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// distance between `track`s in one group,
    /// along the main axis of instrument
    pub key_bands_gap: f32,
    /// distance between `group`s,
    /// along the main axis of instrument
    pub groups_gap: f32,
    /// number of keys and bands in each group
    pub num_keys_per_group: NonZero<u8>,
    /// number of groups
    pub num_groups: NonZero<u8>,
}

impl Eq for Layout {}
