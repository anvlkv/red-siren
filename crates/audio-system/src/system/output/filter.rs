use std::f32;

use common::instrument::{NodeConfig, K_BASE};
use fundsp::prelude::*;

use crate::{
    output::throw_catch::{ThrowCatchCatch, ThrowCatchThrow},
    system::values::FineTunedValue,
    util::DbLin,
    values::FineTunedValues,
};

#[derive(Clone)]
pub struct FilterHandles {
    pub control_a_b: Shared,
    pub control: Shared,
    pub secondary_xct: Shared,
    pub config: NodeConfig,
}

// Smoothed controls.
type Control<S> = Pipe<Var, Follow<S>>;
type ClampedControl<S> = Pipe<
    Pipe<Pipe<Control<S>, Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>>, Shaper<ClipTo>>,
    Binop<FrameSub<U1>, Constant<U1>, Pass>,
>;
type InvertedControl<S> =
    Pipe<Binop<FrameSub<U1>, Constant<U1>, Pipe<Var, Follow<S>>>, Shaper<ClipTo>>;
type QControl<S> = Pipe<
    Pipe<
        Pipe<Pipe<Control<S>, Binop<FrameSub<U1>, ClampedControl<S>, Pass>>, super::abs::Abs>,
        Shaper<ClipTo>,
    >,
    Binop<FrameMul<U1>, Pass, FineTunedValue>,
>;

type FairGain<S> = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Unop<Unop<Var, FrameMulScalar<U1>>, FrameNegAddScalar<U1>>,
                Binop<FrameAdd<U1>, Constant<U1>, Binop<FrameMul<U1>, Pass, ClampedControl<S>>>,
            >,
            Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
        >,
        super::abs::Abs,
    >,
    Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
>;

// Frequency branches

type PassBranch<M, S> = Pipe<
    Stack<Stack<Pass, Binop<FrameMul<U1>, Constant<U1>, InvertedControl<S>>>, QControl<S>>,
    Svf<S, M>,
>;

// High frequency branch
type HpBranch<S> = PassBranch<HighpassMode<S>, S>;
// Band frequency branch
type BpBranch<S> = PassBranch<BandpassMode<S>, S>;
// Low frequency branch
type LpBranch<S> = PassBranch<LowpassMode<S>, S>;

// Controlled split between A/B treatment branches
type PannerControlled<S> = Pipe<Pipe<Stack<Pass, ClampedControl<S>>, Panner<U2>>, Reverse<U2>>;
type PannerBranches<S> =
    Stack<Stack<PannerControlled<S>, PannerControlled<S>>, PannerControlled<S>>;
// Frequency branches
type FreqBranches<S> = Pipe<
    Pipe<Stack<Stack<HpBranch<S>, BpBranch<S>>, LpBranch<S>>, MultiSplit<U3, U2>>,
    Stack<
        Stack<Stack<Stack<Stack<Pass, ThrowCatchThrow>, Pass>, ThrowCatchThrow>, Pass>,
        ThrowCatchThrow,
    >,
>;
type ShelfInput = Stack<Stack<Stack<Pass, Constant<U1>>, Pass>, Pass>;
type FreqCatch<S> = Pipe<
    Pipe<
        Binop<FrameAdd<U1>, Binop<FrameAdd<U1>, ThrowCatchCatch, ThrowCatchCatch>, ThrowCatchCatch>,
        Stack<Stack<Pass, QControl<S>>, FairGain<S>>,
    >,
    Bus<
        Bus<Pipe<ShelfInput, Svf<S, HighshelfMode<S>>>, Pipe<ShelfInput, Svf<S, HighshelfMode<S>>>>,
        Pipe<ShelfInput, Svf<S, LowshelfMode<S>>>,
    >,
>;

// Parameter input to A/B branches
type BranchInput<S> = Stack<Stack<Pass, Constant<U1>>, QControl<S>>;

// A branches
type APassBranch<S> = Pipe<
    Pipe<BranchInput<S>, Stack<Stack<Stack<Pass, Pass>, Pass>, DbLin>>,
    DirtyBiquad<S, BellBiquad<S>, Softsign>,
>;

// B branches
//
// High frequency branch B
type HpBranchB<S> = Pipe<
    Pipe<BranchInput<S>, DirtyBiquad<S, ResonatorBiquad<S>, Crush>>,
    Binop<FrameMul<U1>, Pass, FairGain<S>>,
>;
// Band frequency branch B
type BpBranchB<S> = Pipe<
    Pipe<BranchInput<S>, FbBiquad<S, ResonatorBiquad<S>, SoftCrush>>,
    Binop<FrameMul<U1>, Pass, FairGain<S>>,
>;
// Low frequency branch B
type LpBranchB<S> = Pipe<
    Pipe<BranchInput<S>, DirtyBiquad<S, ResonatorBiquad<S>, SoftCrush>>,
    Binop<FrameMul<U1>, Pass, FairGain<S>>,
>;

// A/B treatment
type AbTreatment<S> = Stack<
    Stack<
        Stack<Stack<Stack<APassBranch<S>, HpBranchB<S>>, APassBranch<S>>, BpBranchB<S>>,
        APassBranch<S>,
    >,
    LpBranchB<S>,
>;

// Treated signal
type WetChain<S> = Pipe<
    Pipe<Pipe<Pipe<Split<U3>, FreqBranches<S>>, PannerBranches<S>>, AbTreatment<S>>,
    Binop<FrameAdd<U1>, Binop<FrameAdd<U1>, Join<U2>, Join<U2>>, Join<U2>>,
>;

// Mix of wet and dry signals
pub type FilterType<S> = Pipe<
    Pipe<
        Pipe<
            Pipe<
                MultiPass<U2>,
                Stack<
                    Binop<FrameMul<U1>, Pass, FairGain<S>>,
                    Binop<FrameMul<U1>, Pass, FairGain<S>>,
                >,
            >,
            Stack<WetChain<S>, Pass>,
        >,
        Stack<Pass, Binop<FrameAdd<U1>, Pass, FreqCatch<S>>>,
    >,
    MultiPass<U2>,
>;

pub fn create_filter<S: Real + Float + 'static>(
    handles: FilterHandles,
    finetuned_values: &FineTunedValues,
    gain: S,
) -> An<FilterType<S>> {
    let FilterHandles {
        control_a_b,
        control,
        config,
        secondary_xct,
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
        (filter_shelf_gain_db) >> super::db_lin::db_lin_converter();

    let control_a_b: An<ClampedControl<S>> = var(&control_a_b)
        >> follow::<S>(S::from_f32(follow_time))
        >> mul(2.0)
        >> clip_to(f32::EPSILON.sqrt(), 2.0 - f32::EPSILON.sqrt())
        >> (constant(1.0) - pass());

    let fair_gain = |v: f32| -> An<FairGain<S>> {
        (1.0 - var(&secondary_xct) * 2.0)
            >> (constant(v.clamp(-1.0, 1.0)) + (pass() * control_a_b.clone()))
            >> mul(0.5)
            >> super::abs::abs()
            >> mul(gain.to_f32())
    };

    let control: An<Control<S>> = var(&control) >> follow::<S>(S::from_f32(follow_time));

    let safe_clip = clip_to(0.001, 0.999);

    let make_q_controlled = |q_value: An<FineTunedValue>| -> An<QControl<S>> {
        control.clone()
            >> (control_a_b.clone() - pass())
            >> super::abs::abs()
            >> safe_clip.clone()
            >> (pass() * q_value)
    };

    let q_piercing_controlled: An<QControl<S>> = make_q_controlled(filter_q_piercing);
    let q_bright_controlled: An<QControl<S>> = make_q_controlled(filter_q_bright);
    let q_shelf_controlled: An<QControl<S>> = make_q_controlled(filter_q_shelf);
    let q_warm_controlled: An<QControl<S>> = make_q_controlled(filter_q_warm);

    let inverted_control: An<InvertedControl<S>> =
        (constant(1.0) - control.clone()) >> safe_clip.clone();

    let hp_branch: An<HpBranch<S>> = (pass()
        | (constant(config.formant_hz(5) as f32) * inverted_control.clone())
        | q_piercing_controlled.clone())
        >> highpass::<S>();

    let bp_branch: An<BpBranch<S>> = (pass()
        | (constant(config.formant_hz(4) as f32) * inverted_control.clone())
        | q_bright_controlled.clone())
        >> bandpass::<S>();

    let lp_branch: An<LpBranch<S>> = (pass()
        | (constant(config.formant_hz(3) as f32) * inverted_control.clone())
        | q_warm_controlled.clone())
        >> lowpass::<S>();

    let shape: f32 = S::from_f64(
        (1.0 / ((config.cents + f64::EPSILON.sqrt()) / 1200.0)).clamp(f64::EPSILON.sqrt(), 1.0),
    )
    .to_f32();

    let hp_input: An<BranchInput<S>> =
        pass() | constant(config.frequency as f32) | q_piercing_controlled.clone();
    let a_hp_branch: An<APassBranch<S>> = hp_input.clone()
        >> (pass() | pass() | pass() | filter_shelf_gain_lin.clone())
        >> dbell(Softsign(shape));
    let b_hp_branch: An<HpBranchB<S>> =
        hp_input >> dresonator(Crush(shape)) >> (pass() * fair_gain(-0.2));

    let mid_f = (config.formant_hz(3) + config.formant_hz(5)) / 2.0;
    let bp_input: An<BranchInput<S>> =
        pass() | constant(mid_f as f32) | q_bright_controlled.clone();

    let a_bp_branch: An<APassBranch<S>> = bp_input.clone()
        >> (pass() | pass() | pass() | filter_shelf_gain_lin.clone())
        >> dbell(Softsign(shape));
    let b_bp_branch: An<BpBranchB<S>> =
        bp_input >> fresonator(SoftCrush(shape)) >> (pass() * fair_gain(0.1));

    let mass = S::from_f64(config.cents.clamp(f64::EPSILON.sqrt(), 1200.0).powf(1.05));
    let hr_bpm = S::from_f32(K_BASE) * mass.pow(S::from_f32(-0.25));
    let hr_hz = hr_bpm / S::from_f32(60.0);
    let lp_input: An<BranchInput<S>> =
        pass() | constant(hr_hz.to_f32()) | q_warm_controlled.clone();
    let a_lp_branch: An<APassBranch<S>> = lp_input.clone()
        >> (pass() | pass() | pass() | filter_shelf_gain_lin.clone())
        >> dbell(Softsign(shape));
    let b_lp_branch: An<LpBranchB<S>> =
        lp_input >> dresonator(SoftCrush(shape)) >> (pass() * fair_gain(-0.1));

    let (hp_throw, hp_catch) = super::throw_catch::throw_catch(2);
    let (bp_throw, bp_catch) = super::throw_catch::throw_catch(2);
    let (lp_throw, lp_catch) = super::throw_catch::throw_catch(2);

    let freq_branches: An<FreqBranches<S>> = (hp_branch | bp_branch | lp_branch)
        >> multisplit::<U3, U2>()
        >> (pass() | hp_throw | pass() | bp_throw | pass() | lp_throw);

    // bypass ab_treatment
    let hs1_input: An<ShelfInput> = pass() | constant(config.frequency as f32) | pass() | pass();
    let hs2_input: An<ShelfInput> = pass() | constant(mid_f as f32) | pass() | pass();
    let ls_input: An<ShelfInput> = pass() | constant(config.formant_hz(2) as f32) | pass() | pass();

    let freq_catch: An<FreqCatch<S>> = (hp_catch + bp_catch + lp_catch)
        >> (pass() | q_shelf_controlled.clone() | fair_gain(-0.3))
        >> ((hs1_input >> highshelf::<S>())
            & (hs2_input >> highshelf::<S>())
            & (ls_input >> lowshelf::<S>()));

    let panner_branch: An<PannerControlled<S>> =
        (pass() | control_a_b.clone()) >> panner() >> reverse::<U2>();

    let panner_branches: An<PannerBranches<S>> =
        panner_branch.clone() | panner_branch.clone() | panner_branch;

    let ab_treatment: An<AbTreatment<S>> =
        a_hp_branch | b_hp_branch | a_bp_branch | b_bp_branch | a_lp_branch | b_lp_branch;

    let wet_chain: An<WetChain<S>> = split::<U3>()
        >> freq_branches
        >> panner_branches
        >> ab_treatment
        >> (join::<U2>() + join::<U2>() + join::<U2>());

    multipass::<U2>()
        >> ((pass() * fair_gain(-0.7)) | (pass() * fair_gain(0.3)))
        >> (wet_chain | pass())
        >> (pass() | (pass() + freq_catch))
        >> multipass::<U2>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::output::prepare_handles;
    use common::instrument::config_test_cases;
    use insta_fun::prelude::*;

    #[test]
    fn a_b_chain() {
        let node = common::instrument::config::NodeConfig::new_test_node(440.0);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&crate::values::FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let control = shared(0.25);

        let control_a_b = shared(0.0);

        let handles = FilterHandles {
            control_a_b: control_a_b.clone(),
            control: control.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_a = create_filter(handles, &values, 1.7);

        let control_a_b = shared(1.0);

        let handles = FilterHandles {
            control_a_b: control_a_b.clone(),
            control: control.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_b = create_filter(handles, &values, 1.7);

        let schema = sine_hz::<f32>(440.0) >> split::<U4>() >> (filter_a | filter_b);

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
        let values = FineTunedValues::new(&crate::values::FineTunedSharedValues::default());
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
            control_a_b: control_a.clone(),
            control: control_0.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_0 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_a.clone(),
            control: control_01.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_01 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_a.clone(),
            control: control_02.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_02 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_a.clone(),
            control: control_05.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_05 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_a.clone(),
            control: control_075.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_075 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_a.clone(),
            control: control_100.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_100 = create_filter(handles, &values, 1.7);

        let schema = sine_hz::<f32>(440.0)
            >> split::<U12>()
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
        let values = FineTunedValues::new(&crate::values::FineTunedSharedValues::default());
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
            control_a_b: control_b.clone(),
            control: control_0.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_0 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_b.clone(),
            control: control_01.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_01 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_b.clone(),
            control: control_02.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_02 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_b.clone(),
            control: control_05.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_05 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_b.clone(),
            control: control_075.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_075 = create_filter(handles, &values, 1.7);

        let handles = FilterHandles {
            control_a_b: control_b.clone(),
            control: control_100.clone(),
            secondary_xct: shared(0.0),
            config: node,
        };

        let filter_100 = create_filter(handles, &values, 1.7);

        let schema = sine_hz::<f32>(440.0)
            >> split::<U12>()
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
    fn config_test_cases_one_by_one() {
        let mut svg_config_bldr = SvgChartConfigBuilder::default();
        svg_config_bldr.show_grid(true);
        svg_config_bldr.preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit());

        let mut snapshot_config_bldr = SnapshotConfigBuilder::default();
        snapshot_config_bldr.num_samples(2000);
        snapshot_config_bldr.warm_up(WarmUp::Seconds(0.5));
        snapshot_config_bldr.allow_abnormal_samples(true);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&crate::values::FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        for (config, layout) in config_test_cases() {
            let mut svg_config_bldr = svg_config_bldr.clone();

            let (node_handles, _, filter_handles) = prepare_handles(&config.0, config.1);
            let mut net = Net::new(0, filter_handles.len());

            for handle in filter_handles {
                let f = handle.config.frequency;
                let key = handle.config.key;

                svg_config_bldr.output_title(format!("{f:.0}Hz_g{}_k{}", key.0, key.1));

                let filter = create_filter(handle, &values, 1.7);
                let node = saw_hz(f as f32) >> split::<U2>() >> filter;

                let id = net.push(Box::new(node));

                net.pipe_output(id);
            }

            node_handles.iter().for_each(|n| {
                n.siren_control.set_value((0.25, 0.1));
                n.band_control.set_value(0.25);
            });

            let snapshot_config = snapshot_config_bldr
                .clone()
                .chart_title(format!(
                    "filter_one_by_one_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                ))
                .try_output_mode(svg_config_bldr)
                .unwrap()
                .build()
                .unwrap();

            assert_audio_unit_snapshot!(net, snapshot_config)
        }
    }
}
