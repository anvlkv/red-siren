#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

#[cfg(not(feature = "editor"))]
pub type FineTunedValue = Constant<U1>;
#[cfg(feature = "editor")]
pub type FineTunedValue = Var;

#[derive(Clone)]
pub struct FineTunedValues {
    pub siren_alpha: An<FineTunedValue>,
    pub filter_morph_follow_s: An<FineTunedValue>,
    pub node_follow_response_time_s: An<FineTunedValue>,
    pub group_q: An<FineTunedValue>,
    pub group_ls_gain_db: An<FineTunedValue>,
    pub filter_q_piercing: An<FineTunedValue>,
    pub filter_q_bright: An<FineTunedValue>,
    pub filter_q_shelf: An<FineTunedValue>,
    pub filter_shelf_gain_db: An<FineTunedValue>,
    pub filter_q_warm: An<FineTunedValue>,
    pub node_bell_q: An<FineTunedValue>,
    pub node_bell_gain_db: An<FineTunedValue>,
    pub formant_base_q: An<FineTunedValue>,
    pub input_ny_threshold: An<FineTunedValue>,
    pub input_ny_wet_ratio: An<FineTunedValue>,
}

#[cfg(feature = "editor")]
pub struct FineTunedSharedValues {
    pub siren_alpha: Shared,
    pub filter_morph_follow_s: Shared,
    pub node_follow_response_time_s: Shared,
    pub group_q: Shared,
    pub group_ls_gain_db: Shared,
    pub filter_q_piercing: Shared,
    pub filter_q_bright: Shared,
    pub filter_q_shelf: Shared,
    pub filter_shelf_gain_db: Shared,
    pub filter_q_warm: Shared,
    pub node_bell_q: Shared,
    pub node_bell_gain_db: Shared,
    pub formant_base_q: Shared,
    pub input_ny_threshold: Shared,
    pub input_ny_wet_ratio: Shared,
}

const SIREN_ALPHA: f32 = 100.0 / 7.5;
const NODE_BELL_Q: f32 = 0.09;
const NODE_BELL_GAIN_DB: f32 = 5.9;
const NODE_FOLLOW_RESPONSE_TIME_S: f32 = 0.175;
const GROUP_Q: f32 = 0.085;
const GROUP_LS_GAIN_DB: f32 = 3.8;
const FORMANT_BASE_Q: f32 = 3.7;
const FILTER_MORPH_FOLLOW_S: f32 = 0.05;
const FILTER_Q_PIERCING: f32 = 1.2;
const FILTER_Q_BRIGHT: f32 = 1.05;
const FILTER_Q_SHELF: f32 = 1.85;
const FILTER_SHELF_GAIN_DB: f32 = 1.3;
const FILTER_Q_WARM: f32 = 1.95;
const INPUT_NY_THRESHOLD: f32 = 0.12;
const INPUT_NY_WET_RATIO: f32 = 0.3;

#[cfg(feature = "editor")]
impl Default for FineTunedSharedValues {
    fn default() -> Self {
        Self {
            siren_alpha: shared(SIREN_ALPHA),
            group_q: shared(GROUP_Q),
            group_ls_gain_db: shared(GROUP_LS_GAIN_DB),
            filter_morph_follow_s: shared(FILTER_MORPH_FOLLOW_S),
            filter_q_piercing: shared(FILTER_Q_PIERCING),
            filter_q_bright: shared(FILTER_Q_BRIGHT),
            filter_q_shelf: shared(FILTER_Q_SHELF),
            filter_shelf_gain_db: shared(FILTER_SHELF_GAIN_DB),
            filter_q_warm: shared(FILTER_Q_WARM),
            node_follow_response_time_s: shared(NODE_FOLLOW_RESPONSE_TIME_S),
            node_bell_q: shared(NODE_BELL_Q),
            node_bell_gain_db: shared(NODE_BELL_GAIN_DB),
            formant_base_q: shared(FORMANT_BASE_Q),
            input_ny_threshold: shared(INPUT_NY_THRESHOLD),
            input_ny_wet_ratio: shared(INPUT_NY_WET_RATIO),
        }
    }
}

#[allow(clippy::new_without_default)]
impl FineTunedValues {
    #[cfg(feature = "editor")]
    pub fn new(shared_values: &FineTunedSharedValues) -> Self {
        Self {
            siren_alpha: var(&shared_values.siren_alpha),
            group_q: var(&shared_values.group_q),
            group_ls_gain_db: var(&shared_values.group_ls_gain_db),
            filter_morph_follow_s: var(&shared_values.filter_morph_follow_s),
            filter_q_piercing: var(&shared_values.filter_q_piercing),
            filter_q_bright: var(&shared_values.filter_q_bright),
            filter_q_shelf: var(&shared_values.filter_q_shelf),
            filter_shelf_gain_db: var(&shared_values.filter_shelf_gain_db),
            filter_q_warm: var(&shared_values.filter_q_warm),
            node_follow_response_time_s: var(&shared_values.node_follow_response_time_s),
            node_bell_q: var(&shared_values.node_bell_q),
            node_bell_gain_db: var(&shared_values.node_bell_gain_db),
            formant_base_q: var(&shared_values.formant_base_q),
            input_ny_threshold: var(&shared_values.input_ny_threshold),
            input_ny_wet_ratio: var(&shared_values.input_ny_wet_ratio),
        }
    }

    #[cfg(not(feature = "editor"))]
    pub fn new() -> Self {
        Self {
            siren_alpha: constant(SIREN_ALPHA),
            group_q: constant(GROUP_Q),
            group_ls_gain_db: constant(GROUP_LS_GAIN_DB),
            filter_morph_follow_s: constant(FILTER_MORPH_FOLLOW_S),
            filter_q_piercing: constant(FILTER_Q_PIERCING),
            filter_q_bright: constant(FILTER_Q_BRIGHT),
            filter_q_shelf: constant(FILTER_Q_SHELF),
            filter_shelf_gain_db: constant(FILTER_SHELF_GAIN_DB),
            filter_q_warm: constant(FILTER_Q_WARM),
            node_follow_response_time_s: constant(NODE_FOLLOW_RESPONSE_TIME_S),
            node_bell_q: constant(NODE_BELL_Q),
            node_bell_gain_db: constant(NODE_BELL_GAIN_DB),
            formant_base_q: constant(FORMANT_BASE_Q),
            input_ny_threshold: constant(INPUT_NY_THRESHOLD),
            input_ny_wet_ratio: constant(INPUT_NY_WET_RATIO),
        }
    }
}

impl std::fmt::Debug for FineTunedValues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FineTunedValues")
            .field("siren_alpha", &self.siren_alpha.value())
            .field("group_q", &self.group_q.value())
            .field("group_ls_gain_db", &self.group_ls_gain_db.value())
            .field("filter_morph_follow_s", &self.filter_morph_follow_s.value())
            .field("filter_q_piercing", &self.filter_q_piercing.value())
            .field("filter_q_bright", &self.filter_q_bright.value())
            .field("filter_q_shelf", &self.filter_q_shelf.value())
            .field("filter_shelf_gain_db", &self.filter_shelf_gain_db.value())
            .field("filter_q_warm", &self.filter_q_warm.value())
            .field(
                "node_follow_response_time_s",
                &self.node_follow_response_time_s.value(),
            )
            .field("node_bell_q", &self.node_bell_q.value())
            .field("node_bell_gain_db", &self.node_bell_gain_db.value())
            .field("formant_base_q", &self.formant_base_q.value())
            .field("input_ny_threshold", &self.input_ny_threshold.value())
            .field("input_ny_wet_ratio", &self.input_ny_wet_ratio.value())
            .finish()
    }
}
