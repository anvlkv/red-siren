/// The array follows CSS box model convention: [top, right, bottom, left]
/// When calculating layout positions:
/// - For main axis (primary direction): uses [0] and [2] (left/right for horizontal, top/bottom for vertical)
/// - For cross axis (secondary direction): uses [1] and [3] (top/bottom for horizontal, left/right for vertical)
pub type SafeArea = [f32; 4];
