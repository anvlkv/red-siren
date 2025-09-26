/// The array follows CSS box model convention: [top, right, bottom, left]
/// When calculating layout positions:
/// - For main axis (primary direction): uses [0] and [2] (left/right for horizontal, top/bottom for vertical)
/// - For cross axis (secondary direction): uses [1] and [3] (top/bottom for horizontal, left/right for vertical)
pub type SafeArea = [f32; 4];

/// Default safe area insets
pub const DEFAULT_SAFE_AREA: f32 = 18.0;

/// Horizontal safe area insets
pub fn horizontal(
    top_safe_area: f32,
    right_safe_area: f32,
    bottom_safe_area: f32,
    left_safe_area: f32,
) -> SafeArea {
    [
        left_safe_area,
        top_safe_area,
        right_safe_area,
        bottom_safe_area,
    ]
}

/// Vertical safe area insets
pub fn vertical(
    top_safe_area: f32,
    right_safe_area: f32,
    bottom_safe_area: f32,
    left_safe_area: f32,
) -> SafeArea {
    [
        top_safe_area,
        left_safe_area,
        bottom_safe_area,
        right_safe_area,
    ]
}
