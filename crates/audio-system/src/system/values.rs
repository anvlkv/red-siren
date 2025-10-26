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
#[derive(Default)]
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
const SIREN_MAX_FREQUENCY_HZ: f32 = 775.0;
const SIREN_EXCITEMENT_PAUSE_LIMIT: f32 = 0.4;
const SIREN_BASE_PAUSE_DURATION: f32 = 0.25;
const FILTER_SWITCH_FOLLOW_RESPONSE_S: f32 = 0.04;
const FILTER_ALLPASS_Q: f32 = 0.6;
const FILTER_ALLPASS_FREQ_RATIO: f32 = 1.0;
const FILTER_MOOG_Q: f32 = 0.03;
const FILTER_MOOG_FREQ_RATIO: f32 = 1.0;
const NODE_FOLLOW_RESPONSE_TIME_S: f32 = 0.33333334;
const NODE_BELL_Q: f32 = 0.085;
const NODE_BELL_GAIN_DB: f32 = 0.33333333;
const FORMANT_BASE_Q: f32 = 0.9;
const INPUT_NY_THRESHOLD: f32 = 0.3;
const INPUT_NY_RATIO: f32 = 4.0;
const INPUT_NY_WET_RATIO: f32 = 0.5;

impl FineTunedValues {
    #[cfg(feature = "editor")]
    pub fn new(shared_values: &FineTunedSharedValues) -> Self {
        shared_values.siren_base_hz.set_value(SIREN_BASE_HZ);
        shared_values
            .siren_max_frequency_hz
            .set_value(SIREN_MAX_FREQUENCY_HZ);
        shared_values
            .siren_excitement_pause_limit
            .set_value(SIREN_EXCITEMENT_PAUSE_LIMIT);
        shared_values
            .siren_base_pause_duration
            .set_value(SIREN_BASE_PAUSE_DURATION);
        shared_values
            .filter_switch_follow_response_s
            .set_value(FILTER_SWITCH_FOLLOW_RESPONSE_S);
        shared_values.filter_allpass_q.set_value(FILTER_ALLPASS_Q);
        shared_values
            .filter_allpass_freq_ratio
            .set_value(FILTER_ALLPASS_FREQ_RATIO);
        shared_values.filter_moog_q.set_value(FILTER_MOOG_Q);
        shared_values
            .filter_moog_freq_ratio
            .set_value(FILTER_MOOG_FREQ_RATIO);

        shared_values
            .node_follow_response_time_s
            .set_value(NODE_FOLLOW_RESPONSE_TIME_S);
        shared_values.node_bell_q.set_value(NODE_BELL_Q);
        shared_values.node_bell_gain_db.set_value(NODE_BELL_GAIN_DB);
        shared_values.formant_base_q.set_value(FORMANT_BASE_Q);
        shared_values
            .input_ny_threshold
            .set_value(INPUT_NY_THRESHOLD);
        shared_values.input_ny_ratio.set_value(INPUT_NY_RATIO);
        shared_values
            .input_ny_wet_ratio
            .set_value(INPUT_NY_WET_RATIO);

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
