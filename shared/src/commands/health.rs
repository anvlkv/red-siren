use serde::{Deserialize, Serialize};

/// Signals that the GUI is ready.
pub const GUI_READY: &str = "health_on_gui_ready";
/// Signals that microphone permission grant has been requested.
pub const GRANT_MIC_PREMISSION: &str = "health_grant_mic_premission";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicPermissionPayload {
    pub prompt: bool,
}
