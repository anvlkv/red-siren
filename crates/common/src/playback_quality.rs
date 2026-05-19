use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackQuality {
    Auto(i8),
    LoFi,
    Medium,
    HiFi,
    Ultra,
}

impl Default for PlaybackQuality {
    fn default() -> Self {
        Self::Auto(0)
    }
}
