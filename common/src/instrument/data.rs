use serde::{Deserialize, Serialize};

/// Signals that microphone permission grant has been requested.
pub const GET_STRING_SNOOP_DATA: &str = "instrument_string_snoop_data";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringSnoopDataResponse {
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringSnoopDataRequest {
    pub group: usize,
    pub key: usize,
}
