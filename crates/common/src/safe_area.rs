use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
/// Distances from screen edges to safe area
pub struct SafeArea {
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
    pub left: i64,
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
pub const DEFAULT_SAFE_AREA: i64 = 18;
