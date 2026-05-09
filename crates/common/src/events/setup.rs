/// Window size (width and height) changed
pub use crate::commands::setup::UPDATE_WINDOW_SIZE;

/// Window appearance (dark mode or light mode) changed
pub use crate::commands::setup::UPDATE_WINDOW_APPEARANCE;

/// Window appearance (dark mode or light mode) changed
pub use crate::commands::setup::GET_WINDOW_APPEARANCE_OVERRIDE;

/// Safe area insets (UI layer) applied to instrument layout (absolute override, not incremental)
pub const UI_SAFE_AREA_INSETS_APPLY: &str = "ui_safe_area_insets_apply";
