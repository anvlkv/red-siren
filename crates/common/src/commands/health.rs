use serde::{Deserialize, Serialize};

/// Signals that the GUI is ready.
pub const GUI_READY: &str = "health_on_gui_ready";
/// Signals that microphone permission grant has been requested.
pub const GRANT_MIC_PERMISSION: &str = "health_grant_mic_permission";
/// Gets the current setup state including mic permission status.
pub const SETUP_STATE: &str = "health_setup_state";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicPermissionPayload {
    pub prompt: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatePayload {
    pub gui_ready: bool,
    pub initial_mic_permission: Option<bool>,
    pub devtools: bool,
}
