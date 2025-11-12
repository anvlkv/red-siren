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
    pub siren_beta: An<FineTunedValue>,
    pub siren_gamma: An<FineTunedValue>,
    pub filter_switch_follow_response_s: An<FineTunedValue>,
    pub node_follow_response_time_s: An<FineTunedValue>,
    pub group_q: An<FineTunedValue>,
    pub group_ls_gain: An<FineTunedValue>,
    pub filter_allpass_q: An<FineTunedValue>,
    pub filter_moog_q: An<FineTunedValue>,
    pub filter_shelf_q: An<FineTunedValue>,
    pub filter_shelf_gain: An<FineTunedValue>,
    pub filter_pass_q: An<FineTunedValue>,
    pub node_bell_q: An<FineTunedValue>,
    pub node_bell_gain_db: An<FineTunedValue>,
    pub formant_base_q: An<FineTunedValue>,
    pub input_ny_threshold: An<FineTunedValue>,
    pub input_ny_wet_ratio: An<FineTunedValue>,
}

#[cfg(feature = "editor")]
pub struct FineTunedSharedValues {
    pub siren_alpha: Shared,
    pub siren_beta: Shared,
    pub siren_gamma: Shared,
    pub filter_switch_follow_response_s: Shared,
    pub node_follow_response_time_s: Shared,
    pub group_q: Shared,
    pub group_ls_gain: Shared,
    pub filter_allpass_q: Shared,
    pub filter_moog_q: Shared,
    pub filter_shelf_q: Shared,
    pub filter_shelf_gain: Shared,
    pub filter_pass_q: Shared,
    pub node_bell_q: Shared,
    pub node_bell_gain_db: Shared,
    pub formant_base_q: Shared,
    pub input_ny_threshold: Shared,
    pub input_ny_wet_ratio: Shared,
}

const SIREN_ALPHA: f32 = 100.0 / 75.0;
const SIREN_BETA: f32 = 0.5;
const SIREN_GAMMA: f32 = 0.075;
const NODE_BELL_Q: f32 = 0.85;
const NODE_BELL_GAIN_DB: f32 = 1.8;
const NODE_FOLLOW_RESPONSE_TIME_S: f32 = 0.34;
const GROUP_Q: f32 = 0.085;
const GROUP_LS_GAIN: f32 = 2.8;
const FORMANT_BASE_Q: f32 = 0.8;
const FILTER_SWITCH_FOLLOW_RESPONSE_S: f32 = 0.04;
const FILTER_ALLPASS_Q: f32 = 0.19;
const FILTER_MOOG_Q: f32 = 0.085;
const FILTER_SHELF_Q: f32 = 0.05;
const FILTER_SHELF_GAIN: f32 = 1.0;
const FILTER_PASS_Q: f32 = 0.04;
const INPUT_NY_THRESHOLD: f32 = 0.07;
const INPUT_NY_WET_RATIO: f32 = 0.3;

#[cfg(feature = "editor")]
impl Default for FineTunedSharedValues {
    fn default() -> Self {
        Self {
            siren_alpha: shared(SIREN_ALPHA),
            siren_beta: shared(SIREN_BETA),
            siren_gamma: shared(SIREN_GAMMA),
            group_q: shared(GROUP_Q),
            group_ls_gain: shared(GROUP_LS_GAIN),
            filter_switch_follow_response_s: shared(FILTER_SWITCH_FOLLOW_RESPONSE_S),
            filter_allpass_q: shared(FILTER_ALLPASS_Q),
            filter_moog_q: shared(FILTER_MOOG_Q),
            filter_shelf_q: shared(FILTER_SHELF_Q),
            filter_shelf_gain: shared(FILTER_SHELF_GAIN),
            filter_pass_q: shared(FILTER_PASS_Q),
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
            siren_beta: var(&shared_values.siren_beta),
            siren_gamma: var(&shared_values.siren_gamma),
            group_q: var(&shared_values.group_q),
            group_ls_gain: var(&shared_values.group_ls_gain),
            filter_switch_follow_response_s: var(&shared_values.filter_switch_follow_response_s),
            filter_allpass_q: var(&shared_values.filter_allpass_q),
            filter_moog_q: var(&shared_values.filter_moog_q),
            filter_shelf_q: var(&shared_values.filter_shelf_q),
            filter_shelf_gain: var(&shared_values.filter_shelf_gain),
            filter_pass_q: var(&shared_values.filter_pass_q),
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
            siren_beta: constant(SIREN_BETA),
            siren_gamma: constant(SIREN_GAMMA),
            group_q: constant(GROUP_Q),
            group_ls_gain: constant(GROUP_LS_GAIN),
            filter_switch_follow_response_s: constant(FILTER_SWITCH_FOLLOW_RESPONSE_S),
            filter_allpass_q: constant(FILTER_ALLPASS_Q),
            filter_moog_q: constant(FILTER_MOOG_Q),
            filter_shelf_q: constant(FILTER_SHELF_Q),
            filter_shelf_gain: constant(FILTER_SHELF_GAIN),
            filter_pass_q: constant(FILTER_PASS_Q),
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
            .field("siren_beta", &self.siren_beta.value())
            .field("siren_gamma", &self.siren_gamma.value())
            .field("group_q", &self.group_q.value())
            .field("group_ls_gain", &self.group_ls_gain.value())
            .field(
                "filter_switch_follow_response_s",
                &self.filter_switch_follow_response_s.value(),
            )
            .field("filter_allpass_q", &self.filter_allpass_q.value())
            .field("filter_moog_q", &self.filter_moog_q.value())
            .field("filter_shelf_q", &self.filter_shelf_q.value())
            .field("filter_shelf_gain", &self.filter_shelf_gain.value())
            .field("filter_pass_q", &self.filter_pass_q.value())
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
