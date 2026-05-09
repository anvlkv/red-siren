/// Line between two points
pub type Line = (mint::Point2<f64>, mint::Point2<f64>);

/// Rectangle from start of coordinates
///
/// - start `mint::Point2<f64>`
/// - size `mint::Vector2<f64>`
pub type Rect = (mint::Point2<f64>, mint::Vector2<f64>);
