pub mod attack;
pub mod config;
pub mod device;
pub mod error;
pub mod playback_quality;
pub mod safe_area;

#[cfg(feature = "egui")]
pub mod egui_helpers;

#[cfg(any(test, feature = "test-util"))]
pub mod test_util;

/// Line between two points
pub type Line = (
    nalgebra::geometry::Point2<i64>,
    nalgebra::geometry::Point2<i64>,
);

/// Rectangle from start of coordinates
///
/// - start `nalgebra::geometry::Point2<i64>`
/// - size `nalgebra::base::Vector2<i64>`
pub type Rect = (
    nalgebra::geometry::Point2<i64>,
    nalgebra::base::Vector2<i64>,
);
