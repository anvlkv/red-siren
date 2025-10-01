/// Window size (width and height) changed
pub use crate::commands::setup::UPDATE_WINDOW_SIZE;

/// Window appearance (dark mode or light mode) changed
pub use crate::commands::setup::UPDATE_WINDOW_APPEARANCE;

/// Window appearance (dark mode or light mode) changed
pub use crate::commands::setup::GET_WINDOW_APPEARANCE_OVERRIDE;

/// Safe area insets applied to instrument layout, originating from ui elements
pub const UI_SAFE_AREA_INSETS_INCREMENT: &str = "ui_safe_area_insets_increment";

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeAreaInstestUiIncrementPayload {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
