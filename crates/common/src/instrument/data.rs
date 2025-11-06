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

pub const GET_ALL_STRING_SNOOPS: &str = "instrument_all_string_snoops";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringSnoopEntry {
    pub group: u8,
    pub key: u8,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringSnoopBatchPayload {
    pub t_unix_ms: u64,
    pub snoops: Vec<StringSnoopEntry>,
}

pub const GET_ACTIVATION_SNOOP_DATA: &str = "instrument_excitement_snoop_data";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcitementSnoopDataResponse {
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcitementSnoopDataRequest {
    pub group: usize,
    pub key: usize,
}

pub const GET_ALL_ACTIVATION_SNOOPS: &str = "instrument_all_excitement_snoops";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcitementSnoopEntry {
    pub group: u8,
    pub key: u8,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcitementSnoopBatchPayload {
    pub t_unix_ms: u64,
    pub snoops: Vec<ExcitementSnoopEntry>,
}
