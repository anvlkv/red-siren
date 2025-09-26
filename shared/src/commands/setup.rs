use serde::{Deserialize, Serialize};

pub const UPDATE_WINDOW_APPEARANCE: &str = "update_window_appearance";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWindowAppearancePayload {
    pub dark: bool,
}
