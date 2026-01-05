use std::num::NonZero;

use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};

use crate::{orientation::LayoutOrientation, safe_area::SafeArea, Line};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Total screen estate available to layout the tuner
    pub space: Vector2<f64>,
    /// Whether `Horizontal` or `Vertical` layout is used,
    /// determines the main and auxilary axes of the tuner layout
    pub orientation: LayoutOrientation,
    /// Minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// Start and end postions of the tuner's analysis line (derived from instrument left string)
    pub line_position: Line,
    /// Radius of each sensor (derived from instrument key radius)
    pub sensor_radius: f64,
    /// Number of sensors (equals number of instrument keys = groups * keys per group)
    pub num_sensors: NonZero<u32>,
}

impl Eq for Layout {}

impl Default for Layout {
    fn default() -> Self {
        Self {
            space: Vector2 { x: 320.0, y: 240.0 },
            orientation: LayoutOrientation::Horizontal,
            safe_area_padding: SafeArea::default(),
            line_position: (Point2 { x: 20.0, y: 120.0 }, Point2 { x: 300.0, y: 120.0 }),
            sensor_radius: 10.0,
            num_sensors: NonZero::new(12).unwrap(),
        }
    }
}

impl From<crate::instrument::Layout> for Layout {
    fn from(value: crate::instrument::Layout) -> Self {
        let total_keys = value.registry().total_keys();

        // Compute spectrum baseline from safe-area: bottom-most for Horizontal, left-most for Vertical
        let baseline = match value.orientation {
            LayoutOrientation::Horizontal => {
                let y = (value.space.y - value.safe_area_padding.bottom - value.key_radius)
                    .clamp(0.0, value.space.y);
                (
                    Point2 { x: 0.0, y },
                    Point2 {
                        x: value.space.x,
                        y,
                    },
                )
            }
            LayoutOrientation::Vertical => {
                let x = (value.safe_area_padding.left + value.key_radius).clamp(0.0, value.space.x);
                (
                    Point2 { x, y: 0.0 },
                    Point2 {
                        x,
                        y: value.space.y,
                    },
                )
            }
        };

        Self {
            space: value.space,
            orientation: value.orientation,
            safe_area_padding: value.safe_area_padding,
            line_position: baseline,
            sensor_radius: value.key_radius,
            num_sensors: NonZero::new(total_keys as u32).expect("total_keys > 0"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::instrument::layout_test_cases;
    use insta::assert_json_snapshot;

    use super::*;

    #[test]
    fn tuner_layout() {
        for layout in layout_test_cases() {
            let tuner_layout: Layout = layout.into();

            assert_json_snapshot!(
                format!("tuner_layout_{}x{}", layout.space.x, layout.space.y),
                tuner_layout
            )
        }
    }
}
