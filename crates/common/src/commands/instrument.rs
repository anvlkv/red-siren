use crate::NodeKey;

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
/// Checks instrument excitement source
pub const GET_ACTIVATION_SRC: &str = "instrument_excitement_source";
/// Sets instrument excitement source
pub const SET_EXCITEMENT_SRC: &str = "instrument_set_excitement_source";
/// Get current instrument layout (same string as event so `use_tauri_resource` can bind both)
pub const GET_LAYOUT: &str = "instrument_layout";
/// Update band control position for a key
pub const UPDATE_BAND_CONTROL: &str = "instrument_update_band_control";
/// Update key control state (pressed/released) for a key
pub const UPDATE_KEY_CONTROL: &str = "instrument_update_key_control";
/// Get batch processing bool
pub const QUALITY_INDICATOR: &str = "instrument_quality_indicator";
/// Get processed output spectrum snapshot
pub const SNAPSHOT_PROCESSED_OUTPUT_SPECTRUM: &str = "snapshot_processed_output_spectrum";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcitementSourcePayload {
    pub source: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBandControlPayload {
    pub keys: Vec<NodeKey>,
    pub increment: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReflectBandControlPayload {
    pub group: u8,
    pub key: u8,
    pub value: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateKeyControlPayload {
    pub keys: Vec<NodeKey>,
    pub value: f32, // 0.0 = released, 1.0 = pressed
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReflectKeyControlPayload {
    pub group: u8,
    pub key: u8,
    pub value: f32, // 0.0 = released, 1.0 = pressed
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumPayload {
    pub t_unix_ms: f64,
    pub data: Vec<(f32, f32)>,
}
