use serde::{Deserialize, Serialize};

use crate::RouteId;

/// Update window appearance (dark mode or light mode)
pub const UPDATE_WINDOW_APPEARANCE: &str = "update_window_appearance";
/// Update window appearance override
pub const WINDOW_APPEARANCE_OVERRIDE: &str = "update_window_appearance_dark_override";
/// Update window size (width and height)
pub const UPDATE_WINDOW_SIZE: &str = "update_window_size";
/// Open page in new window
pub const OPEN_NEW_WIDNOW: &str = "open_in_new_window";
/// Get value of appearance override
pub const GET_WINDOW_APPEARANCE_OVERRIDE: &str = "window_appearance_override";

/// Safe area insets (UI layer) applied to instrument layout, originating from UI elements.
///
/// This value is an absolute override of the UI-contributed safe area portion (NOT an incremental add).
/// Back-end recomputes full layout using these values each time it is applied.
pub const UI_SAFE_AREA_INSETS_APPLY: &str = "ui_safe_area_insets_apply";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWindowAppearancePayload {
    pub dark: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWindowAppearanceOverridePayload {
    pub dark: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWindowSizePayload {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInNewWindowPayload {
    pub route: RouteId,
}
