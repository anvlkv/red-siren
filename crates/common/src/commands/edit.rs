use serde::{Deserialize, Serialize};

pub const EDIT_FINETUNED_VALUES: &str = "instrument_edit_finetuned_values";
pub const GET_FINETUNED_VALUES: &str = "instrument_get_finetuned_values";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FineTunedValuesPayload {
    pub siren_base_hz: f32,
    pub siren_max_frequency_hz: f32,
    pub siren_excitement_pause_limit: f32,
    pub siren_base_pause_duration: f32,
    pub filter_switch_follow_response_s: f32,
    pub node_follow_response_time_s: f32,
    pub filter_base_q: f32,
    pub node_bell_q: f32,
    pub node_bell_gain_db: f32,
    pub formant_base_q: f32,
    pub input_ny_threshold: f32,
    pub input_ny_ratio: f32,
    pub input_ny_wet_ratio: f32,
}
