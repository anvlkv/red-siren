/// Line between two points
pub type Line = (mint::Point2<i64>, mint::Point2<i64>);

/// Rectangle from start of coordinates
///
/// - start `mint::Point2<i64>`
/// - size `mint::Vector2<i64>`
pub type Rect = (mint::Point2<i64>, mint::Vector2<i64>);
