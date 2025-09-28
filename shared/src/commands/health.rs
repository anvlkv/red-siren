use serde::{Deserialize, Serialize};

pub const GUI_READY: &str = "health_on_gui_ready";
pub const GRANT_MIC_PREMISSION: &str = "health_grant_mic_premission";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicPermissionPayload {
    pub prompt: bool,
}
