use std::num::NonZero;

use mint::Vector2;
use serde::{Deserialize, Serialize};

use crate::{orientation::LayoutOrientation, safe_area::SafeArea, Line};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Total screen estate available to layout the tuner
    pub space: Vector2<f32>,
    /// Whether `Horizontal` or `Vertical` layout is used,
    /// determines the main and auxilary axes of the tuner layout
    pub orientation: LayoutOrientation,
    /// Minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// Start and end postions of the tuner's analysis line (derived from instrument left string)
    pub line_position: Line,
    /// Radius of each sensor (derived from instrument key radius)
    pub sensor_radius: f32,
    /// Max range of each sensor (main-axis per-sensor length, cross-axis full breadth)
    pub sensor_max_range: Vector2<f32>,
    /// Number of sensors (equals number of instrument keys = groups * keys per group)
    pub num_sensors: NonZero<u32>,
}

impl Eq for Layout {}

impl From<crate::instrument::Layout> for Layout {
    fn from(value: crate::instrument::Layout) -> Self {
        let total_keys = (value.num_groups.get() as u32) * (value.num_keys_per_group.get() as u32);
        let safe_len = value
            .orientation
            .safe_length(value.space, value.safe_area_padding);
        let safe_breadth = value
            .orientation
            .safe_breadth(value.space, value.safe_area_padding);
        let per_sensor_len = safe_len / (total_keys as f32).max(1.0);
        let sensor_max_range = match value.orientation {
            LayoutOrientation::Horizontal => Vector2 {
                x: per_sensor_len,
                y: safe_breadth,
            },
            LayoutOrientation::Vertical => Vector2 {
                x: safe_breadth,
                y: per_sensor_len,
            },
        };
        Self {
            space: value.space,
            orientation: value.orientation,
            safe_area_padding: value.safe_area_padding,
            line_position: value.left_string_position,
            sensor_radius: value.key_radius,
            sensor_max_range,
            num_sensors: NonZero::new(total_keys).expect("total_keys > 0"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::instrument::layout_test_cases;

    use super::*;

    #[test]
    fn tuner_for_layout_test_cases() {
        for layout in layout_test_cases() {
            let tuner_layout: Layout = layout.into();

            println!("{tuner_layout:#?}");
        }
    }
}
