/// Emited when playback state changes
pub const PLAYBACK_STATE: &str = "instrument_playback_state";
/// Emited when instrument activation source changes
pub const ACTIVATION_SRC: &str = "instrument_activation_source";

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
