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
    pub siren_base_hz: An<FineTunedValue>,
    pub siren_max_frequency_hz: An<FineTunedValue>,
    pub siren_excitement_pause_limit: An<FineTunedValue>,
    pub siren_base_pause_duration: An<FineTunedValue>,
    pub filter_switch_follow_response_s: An<FineTunedValue>,
    pub node_follow_response_time_s: An<FineTunedValue>,
    pub filter_allpass_q: An<FineTunedValue>,
    pub filter_allpass_freq_ratio: An<FineTunedValue>,
    pub filter_moog_freq_ratio: An<FineTunedValue>,
    pub filter_moog_q: An<FineTunedValue>,
    pub node_bell_q: An<FineTunedValue>,
    pub node_bell_gain_db: An<FineTunedValue>,
    pub formant_base_q: An<FineTunedValue>,
    pub input_ny_threshold: An<FineTunedValue>,
    pub input_ny_ratio: An<FineTunedValue>,
    pub input_ny_wet_ratio: An<FineTunedValue>,
}

#[cfg(feature = "editor")]
pub struct FineTunedSharedValues {
    pub siren_base_hz: Shared,
    pub siren_max_frequency_hz: Shared,
    pub siren_excitement_pause_limit: Shared,
    pub siren_base_pause_duration: Shared,
    pub filter_switch_follow_response_s: Shared,
    pub node_follow_response_time_s: Shared,
    pub filter_allpass_q: Shared,
    pub filter_allpass_freq_ratio: Shared,
    pub filter_moog_freq_ratio: Shared,
    pub filter_moog_q: Shared,
    pub node_bell_q: Shared,
    pub node_bell_gain_db: Shared,
    pub formant_base_q: Shared,
    pub input_ny_threshold: Shared,
    pub input_ny_ratio: Shared,
    pub input_ny_wet_ratio: Shared,
}

const SIREN_BASE_HZ: f32 = 0.5;
const SIREN_MAX_FREQUENCY_HZ: f32 = 1775.0;
const SIREN_EXCITEMENT_PAUSE_LIMIT: f32 = 0.4;
const SIREN_BASE_PAUSE_DURATION: f32 = 0.25;
const FILTER_SWITCH_FOLLOW_RESPONSE_S: f32 = 0.04;
const FILTER_ALLPASS_Q: f32 = 0.06;
const FILTER_ALLPASS_FREQ_RATIO: f32 = 0.7;
const FILTER_MOOG_Q: f32 = 0.25;
const FILTER_MOOG_FREQ_RATIO: f32 = 1.3;
const NODE_FOLLOW_RESPONSE_TIME_S: f32 = 0.34;
const NODE_BELL_Q: f32 = 0.085;
const NODE_BELL_GAIN_DB: f32 = 1.34;
const FORMANT_BASE_Q: f32 = 0.8;
const INPUT_NY_THRESHOLD: f32 = 0.3;
const INPUT_NY_RATIO: f32 = 4.0;
const INPUT_NY_WET_RATIO: f32 = 0.5;

#[cfg(feature = "editor")]
impl Default for FineTunedSharedValues {
    fn default() -> Self {
        Self {
            siren_base_hz: shared(SIREN_BASE_HZ),
            siren_max_frequency_hz: shared(SIREN_MAX_FREQUENCY_HZ),
            siren_excitement_pause_limit: shared(SIREN_EXCITEMENT_PAUSE_LIMIT),
            siren_base_pause_duration: shared(SIREN_BASE_PAUSE_DURATION),
            filter_switch_follow_response_s: shared(FILTER_SWITCH_FOLLOW_RESPONSE_S),
            filter_allpass_q: shared(FILTER_ALLPASS_Q),
            filter_allpass_freq_ratio: shared(FILTER_ALLPASS_FREQ_RATIO),
            filter_moog_freq_ratio: shared(FILTER_MOOG_FREQ_RATIO),
            filter_moog_q: shared(FILTER_MOOG_Q),
            node_follow_response_time_s: shared(NODE_FOLLOW_RESPONSE_TIME_S),
            node_bell_q: shared(NODE_BELL_Q),
            node_bell_gain_db: shared(NODE_BELL_GAIN_DB),
            formant_base_q: shared(FORMANT_BASE_Q),
            input_ny_threshold: shared(INPUT_NY_THRESHOLD),
            input_ny_ratio: shared(INPUT_NY_RATIO),
            input_ny_wet_ratio: shared(INPUT_NY_WET_RATIO),
        }
    }
}

#[allow(clippy::new_without_default)]
impl FineTunedValues {
    #[cfg(feature = "editor")]
    pub fn new(shared_values: &FineTunedSharedValues) -> Self {
        Self {
            siren_base_hz: var(&shared_values.siren_base_hz),
            siren_max_frequency_hz: var(&shared_values.siren_max_frequency_hz),
            siren_excitement_pause_limit: var(&shared_values.siren_excitement_pause_limit),
            siren_base_pause_duration: var(&shared_values.siren_base_pause_duration),
            filter_switch_follow_response_s: var(&shared_values.filter_switch_follow_response_s),
            filter_allpass_q: var(&shared_values.filter_allpass_q),
            filter_allpass_freq_ratio: var(&shared_values.filter_allpass_freq_ratio),
            filter_moog_freq_ratio: var(&shared_values.filter_moog_freq_ratio),
            filter_moog_q: var(&shared_values.filter_moog_q),
            node_follow_response_time_s: var(&shared_values.node_follow_response_time_s),
            node_bell_q: var(&shared_values.node_bell_q),
            node_bell_gain_db: var(&shared_values.node_bell_gain_db),
            formant_base_q: var(&shared_values.formant_base_q),
            input_ny_threshold: var(&shared_values.input_ny_threshold),
            input_ny_ratio: var(&shared_values.input_ny_ratio),
            input_ny_wet_ratio: var(&shared_values.input_ny_wet_ratio),
        }
    }

    #[cfg(not(feature = "editor"))]
    pub fn new() -> Self {
        Self {
            siren_base_hz: constant(SIREN_BASE_HZ),
            siren_max_frequency_hz: constant(SIREN_MAX_FREQUENCY_HZ),
            siren_excitement_pause_limit: constant(SIREN_EXCITEMENT_PAUSE_LIMIT),
            siren_base_pause_duration: constant(SIREN_BASE_PAUSE_DURATION),
            filter_switch_follow_response_s: constant(FILTER_SWITCH_FOLLOW_RESPONSE_S),
            filter_allpass_q: constant(FILTER_ALLPASS_Q),
            filter_allpass_freq_ratio: constant(FILTER_ALLPASS_FREQ_RATIO),
            filter_moog_freq_ratio: constant(FILTER_MOOG_FREQ_RATIO),
            filter_moog_q: constant(FILTER_MOOG_Q),
            node_follow_response_time_s: constant(NODE_FOLLOW_RESPONSE_TIME_S),
            node_bell_q: constant(NODE_BELL_Q),
            node_bell_gain_db: constant(NODE_BELL_GAIN_DB),
            formant_base_q: constant(FORMANT_BASE_Q),
            input_ny_threshold: constant(INPUT_NY_THRESHOLD),
            input_ny_ratio: constant(INPUT_NY_RATIO),
            input_ny_wet_ratio: constant(INPUT_NY_WET_RATIO),
        }
    }
}

impl std::fmt::Debug for FineTunedValues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FineTunedValues")
            .field("siren_base_hz", &self.siren_base_hz.value())
            .field(
                "siren_max_frequency_hz",
                &self.siren_max_frequency_hz.value(),
            )
            .field(
                "siren_excitement_pause_limit",
                &self.siren_excitement_pause_limit.value(),
            )
            .field(
                "siren_base_pause_duration",
                &self.siren_base_pause_duration.value(),
            )
            .field(
                "filter_switch_follow_response_s",
                &self.filter_switch_follow_response_s.value(),
            )
            .field("filter_allpass_q", &self.filter_allpass_q.value())
            .field(
                "filter_allpass_freq_ratio",
                &self.filter_allpass_freq_ratio.value(),
            )
            .field(
                "filter_moog_freq_ratio",
                &self.filter_moog_freq_ratio.value(),
            )
            .field("filter_moog_q", &self.filter_moog_q.value())
            .field(
                "node_follow_response_time_s",
                &self.node_follow_response_time_s.value(),
            )
            .field("node_bell_q", &self.node_bell_q.value())
            .field("node_bell_gain_db", &self.node_bell_gain_db.value())
            .field("formant_base_q", &self.formant_base_q.value())
            .field("input_ny_threshold", &self.input_ny_threshold.value())
            .field("input_ny_ratio", &self.input_ny_ratio.value())
            .field("input_ny_wet_ratio", &self.input_ny_wet_ratio.value())
            .finish()
    }
}
