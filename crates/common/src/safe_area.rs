use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Distances from screen edges to safe area
pub struct SafeArea {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
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
pub const DEFAULT_SAFE_AREA: f64 = 18.0;
