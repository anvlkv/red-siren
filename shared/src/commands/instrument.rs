/// Start instrument playback
pub const PLAYBACK_START: &str = "instrument_playback_start";
/// Kill instrument engine
pub const PLAYBACK_STOP: &str = "instrument_playback_stop";
/// Checks instrument playback state
pub const PLAYBACK_GET_STATE: &str = "instrument_playback_state";
/// Pause instrument playback
pub const PLAYBACK_PAUSE: &str = "instrument_playback_pause";
/// Resume instrument playback
pub const PLAYBACK_RESUME: &str = "instrument_playback_resume";
/// Checks instrument activation source
pub const GET_ACTIVATION_SRC: &str = "instrument_activation_source";
/// Sets instrument activation source
pub const SET_ACTIVATION_SRC: &str = "instrument_set_activation_source";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationSourcePayload {
    pub source: u8,
}
