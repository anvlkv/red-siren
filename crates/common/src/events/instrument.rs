/// Emited when playback state changes
pub const PLAYBACK_STATE: &str = "instrument_playback_state";
/// Emited when instrument activation source changes
pub const ACTIVATION_SRC: &str = "instrument_activation_source";
/// Emitted when instrument layout changes (invoke & event share this string)
pub const LAYOUT: &str = "instrument_layout";
/// Emitted when band control position changes
pub const BAND_CONTROL_G_K: &str = "instrument_band_control";
/// Emitted when key control state changes (pressed/released)
pub const KEY_CONTROL_G_K: &str = "instrument_key_control";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatePayload {
    pub playing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationSourcePayload {
    pub source: u8,
}
