use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Distances from screen edges to safe area
pub struct SafeArea {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Default for SafeArea {
    fn default() -> Self {
        Self {
            top: DEFAULT_SAFE_AREA,
            right: DEFAULT_SAFE_AREA,
            bottom: DEFAULT_SAFE_AREA,
            left: DEFAULT_SAFE_AREA,
        }
    }
}

/// Default safe area insets
pub const DEFAULT_SAFE_AREA: f32 = 18.0;
