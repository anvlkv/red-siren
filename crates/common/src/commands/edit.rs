use serde::{Deserialize, Serialize};

pub const EDIT_FINETUNED_VALUES: &str = "instrument_edit_finetuned_values";
pub const GET_FINETUNED_VALUES: &str = "instrument_get_finetuned_values";

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FineTunedValuesPayload {
    pub siren_alpha: f32,
    pub group_q: f32,
    pub group_ls_gain_db: f32,
    pub filter_morph_follow_s: f32,
    pub node_follow_response_time_s: f32,
    pub filter_q_piercing: f32,
    pub filter_q_bright: f32,
    pub filter_q_shelf: f32,
    pub filter_shelf_gain_db: f32,
    pub filter_q_warm: f32,
    pub node_bell_q: f32,
    pub node_bell_gain_db: f32,
    pub formant_base_q: f32,
    pub input_ny_threshold: f32,
    pub input_ny_wet_ratio: f32,
}
