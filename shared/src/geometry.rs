/// Line between two points
pub type Line = (mint::Point2<f32>, mint::Point2<f32>);

/// Rectangle from start of coordinates
///
/// - start `mint::Point2<f32>`
/// - size `mint::Vector2<f32>`
pub type Rect = (mint::Point2<f32>, mint::Vector2<f32>);
