use serde::{Deserialize, Serialize};

/// Update window appearance (dark mode or light mode)
pub const UPDATE_WINDOW_APPEARANCE: &str = "update_window_appearance";
/// Update window size (width and height)
pub const UPDATE_WINDOW_SIZE: &str = "update_window_size";

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
