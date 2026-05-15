mod embodied;
mod revolution_body;
mod segment;
mod thick_body;

pub use embodied::{Embodied, EmbodiedBounds, EmbodiedPoint3, EmbodiedTriangle};
pub use revolution_body::{RevolutionAxis, RevolutionBody, RevolutionBodyError};
pub use segment::*;
pub use thick_body::{ThickBody, ThicknessMap, ThicknessMapPoint};

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
