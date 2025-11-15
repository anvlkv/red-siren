use common::instrument::NodeConfig;
use fundsp::{
    hacker::prelude::*,
    typenum::{UInt, UTerm, B1},
};

use crate::{system::values::FineTunedValue, util::S, values::FineTunedValues};

#[derive(Clone)]
pub struct FilterHandles {
    pub control_a_b: Var,
    pub control: Var,
    pub config: NodeConfig,
}

type Control = Pipe<Var, Follow<S>>;

type PowCents = Pipe<
    Stack<
        Constant<UInt<UTerm, B1>>, // base = 2.0
        Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, Control>, // exponent = (cents/1200)*control
    >,
    super::pow::Pow<S>,
>;

type AllPassChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
        FineTunedValue,
    >,
    Svf<S, AllpassMode<S>>,
>;

type MoogChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
        FineTunedValue,
    >,
    Moog<S, UInt<UInt<UTerm, B1>, B1>>,
>;

type ShelfChain = Pipe<
    Stack<
        Stack<
            Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
            FineTunedValue,
        >,
        FineTunedValue,
    >,
    Svf<S, HighshelfMode<S>>,
>;

type PassChain = Pipe<
    Stack<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<UInt<UTerm, B1>>, PowCents>>,
        FineTunedValue,
    >,
    Svf<S, LowpassMode<S>>,
>;

type CrossFadeChain =
    Pipe<Stack<Stack<Pass, Pass>, Control>, super::crossfade::EqualPowerCrossfade>;

pub type FilterType = Pipe<
    Pipe<Split<U2>, Stack<Pipe<ShelfChain, PassChain>, Pipe<AllPassChain, MoogChain>>>,
    CrossFadeChain,
>;

pub fn create_filter(handles: FilterHandles, finetuned_values: &FineTunedValues) -> An<FilterType> {
    let FilterHandles {
        control_a_b,
        control,
        config,
    } = handles;

    let FineTunedValues {
        filter_switch_follow_response_s,
        filter_allpass_q,
        filter_moog_q,
        filter_shelf_q,
        filter_shelf_gain,
        filter_pass_q,
        ..
    } = finetuned_values.clone();

    // Get the follow response time value
    #[cfg(feature = "editor")]
    let follow_time = filter_switch_follow_response_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = filter_switch_follow_response_s.value()[0];

    let control_a_b: An<Control> = An(control_a_b) >> follow::<S>(follow_time as S);
    let control: An<Control> = An(control) >> follow::<S>(follow_time as S);

    let pow_cents: An<PowCents> = (constant(2.0)
        | (constant((config.cents as f32) / 1200.0) * control.clone()))
        >> super::pow::pow::<S>();

    // B chains
    let filter_allpass_chain: An<AllPassChain> =
        (pass() | (constant(config.formant_hz(4) as f32) * pow_cents.clone()) | filter_allpass_q)
            >> allpass::<S>();

    let filter_moog_chain: An<MoogChain> =
        (pass() | (constant(config.formant_hz(5) as f32) * pow_cents.clone()) | filter_moog_q)
            >> moog::<S>();

    // A chains
    let filter_shelf_chain: An<ShelfChain> = (pass()
        | (constant(config.formant_hz(4) as f32) * pow_cents.clone())
        | filter_shelf_q
        | filter_shelf_gain)
        >> highshelf::<S>();

    let filter_pass_chain: An<PassChain> =
        (pass() | (constant(config.formant_hz(5) as f32) * pow_cents) | filter_pass_q)
            >> lowpass::<S>();

    let cross_fade_chain: An<CrossFadeChain> =
        (pass() | pass() | control_a_b) >> super::crossfade::equal_power_crossfade();

    split::<U2>()
        >> ((filter_shelf_chain >> filter_pass_chain) | (filter_allpass_chain >> filter_moog_chain))
        >> cross_fade_chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    #[test]
    fn a_b_chain() {
        let node = common::instrument::config::NodeConfig::new_test_node(440.0);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let control = shared(0.0);

        let control_a_b = shared(0.0);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control),
            config: node,
        };

        let filter_a = create_filter(handles, &values);

        let control_a_b = shared(1.0);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control),
            config: node,
        };

        let filter_b = create_filter(handles, &values);

        let schema = sine_hz::<f32>(440.0) >> split::<U2>() >> (filter_a | filter_b);

        let config = SnapshotConfigBuilder::default()
            .chart_layout(Layout::Combined)
            .output_title("A Chain")
            .output_title("B Chain")
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(schema, config);
    }

    #[test]
    fn control_values() {
        let node = common::instrument::config::NodeConfig::new_test_node(440.0);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let control_0 = shared(0.0);
        let control_01 = shared(0.1);
        let control_02 = shared(0.2);
        let control_05 = shared(0.5);
        let control_075 = shared(0.75);
        let control_100 = shared(1.0);

        let control_a_b = shared(0.0);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control_0),
            config: node,
        };

        let filter_0 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control_01),
            config: node,
        };

        let filter_01 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control_02),
            config: node,
        };

        let filter_02 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control_05),
            config: node,
        };

        let filter_05 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control_075),
            config: node,
        };

        let filter_075 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a_b),
            control: Var::new(&control_100),
            config: node,
        };

        let filter_100 = create_filter(handles, &values);

        let schema = sine_hz::<f32>(440.0)
            >> split::<U6>()
            >> (filter_0 | filter_01 | filter_02 | filter_05 | filter_075 | filter_100);

        let config = SnapshotConfigBuilder::default()
            .chart_layout(Layout::Combined)
            .output_title("control value: 0.0")
            .output_title("control value: 0.1")
            .output_title("control value: 0.2")
            .output_title("control value: 0.5")
            .output_title("control value: 0.75")
            .output_title("control value: 1.0")
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(schema, config);
    }
}
