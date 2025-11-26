use std::f32;

use common::instrument::{NodeConfig, K_BASE};
use fundsp::{
    hacker::prelude::*,
    typenum::{UInt, UTerm, B0, B1},
};

use crate::{system::values::FineTunedValue, util::S, values::FineTunedValues};

#[derive(Clone)]
pub struct FilterHandles {
    pub control_a_b: Var,
    pub control: Var,
    pub config: NodeConfig,
}

// Smoothed controls.
type Control = Pipe<Var, Follow<S>>;
type ClampedControl = Pipe<
    Pipe<
        Pipe<Control, Binop<FrameMul<UInt<UTerm, B1>>, MultiPass<UInt<UTerm, B1>>, Constant<U1>>>,
        Shaper<ClipTo>,
    >,
    Binop<FrameSub<UInt<UTerm, B1>>, Constant<U1>, Pass>,
>;
type InvertedControl =
    Pipe<Binop<FrameSub<UInt<UTerm, B1>>, Constant<U1>, Pipe<Var, Follow<S>>>, Shaper<ClipTo>>;
type QControl = Pipe<
    Pipe<
        Pipe<
            Pipe<Control, Binop<FrameSub<UInt<UTerm, B1>>, ClampedControl, Pass>>,
            super::abs::Abs,
        >,
        Shaper<ClipTo>,
    >,
    Binop<FrameMul<UInt<UTerm, B1>>, Pass, FineTunedValue>,
>;

type DbLin = Pipe<FineTunedValue, super::db_lin::DbLinConverter>;

type HpBranch = Pipe<
    Pipe<
        Stack<
            Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<U1>, InvertedControl>>,
            QControl,
        >,
        Stack<Stack<Svf<S, HighpassMode<S>>, Constant<U1>>, QControl>,
    >,
    Svf<S, AllpassMode<S>>,
>;

type BpBranch = Pipe<
    Pipe<
        Stack<
            Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<U1>, InvertedControl>>,
            QControl,
        >,
        Stack<
            Stack<Stack<Svf<S, BandpassMode<S>>, Constant<U1>>, QControl>,
            Pipe<FineTunedValue, super::db_lin::DbLinConverter>,
        >,
    >,
    Svf<S, HighshelfMode<S>>,
>;

type LpBranch = Pipe<
    Pipe<
        Stack<Pass, Binop<FrameMul<UInt<UTerm, B1>>, Constant<U1>, InvertedControl>>,
        Stack<
            Stack<Stack<Lowpole<S, UInt<UInt<UTerm, B1>, B0>>, Constant<U1>>, QControl>,
            Pipe<FineTunedValue, super::db_lin::DbLinConverter>,
        >,
    >,
    Svf<S, LowshelfMode<S>>,
>;

type BranchInput = Stack<Stack<Pass, Constant<U1>>, QControl>;

type PannerControlled = Pipe<Stack<Pass, ClampedControl>, Panner<U2>>;

type FreqBranches = Stack<Stack<HpBranch, BpBranch>, LpBranch>;

type PannerBranches = Stack<Stack<PannerControlled, PannerControlled>, PannerControlled>;

type HpBranchA = Pipe<
    Pipe<BranchInput, Stack<Stack<Stack<Pass, Pass>, Pass>, DbLin>>,
    DirtyBiquad<S, BellBiquad<S>, SoftCrush>,
>;
type BpBranchA = Pipe<
    Pipe<BranchInput, Stack<Stack<Stack<Pass, Pass>, Pass>, DbLin>>,
    DirtyBiquad<S, BellBiquad<S>, Softsign>,
>;
type LpBranchA = Pipe<
    Pipe<BranchInput, Stack<Stack<Stack<Pass, Pass>, Pass>, DbLin>>,
    DirtyBiquad<S, BellBiquad<S>, Crush>,
>;

type HpBranchB = Pipe<BranchInput, DirtyBiquad<S, ResonatorBiquad<S>, Tanh>>;
type BpBranchB = Pipe<BranchInput, FbBiquad<S, ResonatorBiquad<S>, Crush>>;
type LpBranchB = Pipe<BranchInput, Rez<S, U3>>;

type AbTreatment = Stack<
    Stack<Stack<Stack<Stack<HpBranchA, HpBranchB>, BpBranchA>, BpBranchB>, LpBranchA>,
    LpBranchB,
>;

type WetChain = Pipe<
    Pipe<
        Pipe<Pipe<Pipe<Pipe<Pinkpass<S>, Split<U3>>, FreqBranches>, PannerBranches>, AbTreatment>,
        Stack<Stack<Join<U2>, Join<U2>>, Join<U2>>,
    >,
    Join<U3>,
>;

pub type FilterType = Pipe<
    Pipe<
        Pipe<
            Split<U2>,
            Stack<
                Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
                Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
            >,
        >,
        Stack<WetChain, Pass>,
    >,
    Join<U2>,
>;

pub fn create_filter(handles: FilterHandles, finetuned_values: &FineTunedValues) -> An<FilterType> {
    let FilterHandles {
        control_a_b,
        control,
        config,
    } = handles;

    let FineTunedValues {
        filter_morph_follow_s,
        filter_q_piercing,
        filter_q_bright,
        filter_q_shelf,
        filter_shelf_gain_db,
        filter_q_warm,
        ..
    } = finetuned_values.clone();

    #[cfg(feature = "editor")]
    let follow_time = filter_morph_follow_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = filter_morph_follow_s.value()[0];

    let filter_shelf_gain_lin: An<DbLin> =
        filter_shelf_gain_db >> super::db_lin::db_lin_converter();

    let control_a_b: An<ClampedControl> = An(control_a_b)
        >> follow::<S>(follow_time as S)
        >> mul(2.0)
        >> clip_to(S::EPSILON.sqrt() as f32, (2.0 - S::EPSILON.sqrt()) as f32)
        >> (constant(1.0) - pass());

    let control: An<Control> = An(control) >> follow::<S>(follow_time as S);

    let make_q_controlled = |q_value: An<FineTunedValue>| -> An<QControl> {
        control.clone()
            >> (control_a_b.clone() - pass())
            >> super::abs::abs()
            >> clip_to(S::EPSILON.sqrt() as f32, (1.0 - S::EPSILON.sqrt()) as f32)
            >> (pass() * q_value)
    };

    let q_piercing_controlled: An<QControl> = make_q_controlled(filter_q_piercing);
    let q_bright_controlled: An<QControl> = make_q_controlled(filter_q_bright);
    let q_shelf_controlled: An<QControl> = make_q_controlled(filter_q_shelf);
    let q_warm_controlled: An<QControl> = make_q_controlled(filter_q_warm);

    let inverted_control: An<InvertedControl> = (constant(1.0) - control.clone())
        >> clip_to(S::EPSILON.sqrt() as f32, (1.0 - S::EPSILON.sqrt()) as f32);

    let hp_branch: An<HpBranch> = (pass()
        | (constant(config.formant_hz(4) as f32) * inverted_control.clone())
        | q_piercing_controlled.clone())
        >> (highpass() | constant(config.formant_hz(5) as f32) | q_bright_controlled.clone())
        >> allpass();

    let bp_branch: An<BpBranch> = (pass()
        | (constant(config.formant_hz(3) as f32) * inverted_control.clone())
        | q_bright_controlled.clone())
        >> (bandpass()
            | constant(config.formant_hz(3) as f32)
            | q_shelf_controlled.clone()
            | filter_shelf_gain_lin.clone())
        >> highshelf();

    let lp_branch: An<LpBranch> = (pass()
        | (constant(config.formant_hz(2) as f32) * inverted_control.clone()))
        >> (lowpole()
            | constant(config.formant_hz(1) as f32)
            | q_shelf_controlled.clone()
            | filter_shelf_gain_lin.clone())
        >> lowshelf();

    let shape = (1.0 / (config.cents + f64::EPSILON.sqrt()) as f32)
        .clamp(f32::EPSILON.sqrt(), 1.0 - f32::EPSILON.sqrt());

    let hp_input: An<BranchInput> =
        pass() | constant(config.frequency as f32) | q_piercing_controlled.clone();
    let a_hp_branch: An<HpBranchA> = hp_input.clone()
        >> (pass() | pass() | pass() | filter_shelf_gain_lin.clone())
        >> dbell(SoftCrush(shape));
    let b_hp_branch: An<HpBranchB> = hp_input >> dresonator(Tanh(shape));

    let mid_f = (config.formant_hz(3) + config.formant_hz(4)) / 2.0;
    let bp_input: An<BranchInput> = pass() | constant(mid_f as f32) | q_bright_controlled.clone();
    let a_bp_branch: An<BpBranchA> = bp_input.clone()
        >> (pass() | pass() | pass() | filter_shelf_gain_lin.clone())
        >> dbell(Softsign(shape));
    let b_bp_branch: An<BpBranchB> = bp_input >> fresonator(Crush(shape));

    let mass = config.cents.powf(1.05) as S;
    let hr_bpm = (K_BASE as S) * mass.powf(-0.25);
    let hr_hz = hr_bpm / 60.0;

    let lp_input: An<BranchInput> = pass() | constant(hr_hz as f32) | q_warm_controlled.clone();
    let a_lp_branch: An<LpBranchA> = lp_input.clone()
        >> (pass() | pass() | pass() | filter_shelf_gain_lin.clone())
        >> dbell(Crush(shape));
    let b_lp_branch: An<LpBranchB> = lp_input >> lowrez();

    let freq_branches: An<FreqBranches> = hp_branch | bp_branch | lp_branch;

    let panner_branch: An<PannerControlled> = (pass() | control_a_b.clone()) >> panner();

    let panner_branches: An<PannerBranches> =
        panner_branch.clone() | panner_branch.clone() | panner_branch;

    let ab_treatment: An<AbTreatment> =
        a_hp_branch | b_hp_branch | a_bp_branch | b_bp_branch | a_lp_branch | b_lp_branch;

    let wet_chain: An<WetChain> = pinkpass::<S>()
        >> split::<U3>()
        >> freq_branches
        >> panner_branches
        >> ab_treatment
        >> (join::<U2>() | join::<U2>() | join::<U2>())
        >> join::<U3>();

    split::<U2>() >> (mul(1.7) | mul(1.3)) >> (wet_chain | pass()) >> join::<U2>()
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

        let control = shared(0.25);

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
    fn control_values_a() {
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

        let control_a = shared(0.0);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a),
            control: Var::new(&control_0),
            config: node,
        };

        let filter_0 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a),
            control: Var::new(&control_01),
            config: node,
        };

        let filter_01 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a),
            control: Var::new(&control_02),
            config: node,
        };

        let filter_02 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a),
            control: Var::new(&control_05),
            config: node,
        };

        let filter_05 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a),
            control: Var::new(&control_075),
            config: node,
        };

        let filter_075 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_a),
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

    #[test]
    fn control_values_b() {
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

        let control_b = shared(1.0);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_b),
            control: Var::new(&control_0),
            config: node,
        };

        let filter_0 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_b),
            control: Var::new(&control_01),
            config: node,
        };

        let filter_01 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_b),
            control: Var::new(&control_02),
            config: node,
        };

        let filter_02 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_b),
            control: Var::new(&control_05),
            config: node,
        };

        let filter_05 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_b),
            control: Var::new(&control_075),
            config: node,
        };

        let filter_075 = create_filter(handles, &values);

        let handles = FilterHandles {
            control_a_b: Var::new(&control_b),
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
