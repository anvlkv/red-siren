mod segment;

pub use segment::*;

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
