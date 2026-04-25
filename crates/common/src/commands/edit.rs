use serde::{Deserialize, Serialize};

pub const EDIT_FINETUNED_VALUES: &str = "instrument_edit_finetuned_values";
pub const GET_FINETUNED_VALUES: &str = "instrument_get_finetuned_values";

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FineTunedValuesPayload {
    pub formants_q: f32,
}
