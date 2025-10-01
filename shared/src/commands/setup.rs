use serde::{Deserialize, Serialize};

/// Update window appearance (dark mode or light mode)
pub const UPDATE_WINDOW_APPEARANCE: &str = "update_window_appearance";
/// Update window size (width and height)
pub const UPDATE_WINDOW_SIZE: &str = "update_window_size";

/// Safe area insets applied to instrument layout, originating from ui elements
pub const UI_SAFE_AREA_INSETS_INCREMENT: &str = "ui_safe_area_insets_increment";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWindowAppearancePayload {
    pub dark: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWindowSizePayload {
    pub width: f64,
    pub height: f64,
}

pub use crate::events::setup::SafeAreaInstestUiIncrementPayload;
