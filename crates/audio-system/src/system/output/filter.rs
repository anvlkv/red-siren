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

// Smoothed controls.
type Control = Pipe<Var, Follow<S>>;
type ClampedControl = Pipe<Pipe<Var, Shaper<ClipTo>>, Follow<S>>;
type DbLin = Pipe<FineTunedValue, super::db_lin::DbLinConverter>;

// Lowshelf node with constant cutoff, finetuned Q, and finetuned gain.
type LSChain = Pipe<
    Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, DbLin>,
    Svf<S, LowshelfMode<S>>,
>;

// Lowshelf node with constant cutoff, finetuned Q, and computed reciprocal gain (for cuts).
type LSChainCut = Pipe<
    Stack<
        Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>,
        Pipe<Stack<Constant<UInt<UTerm, B1>>, DbLin>, super::div::Div<S>>,
    >,
    Svf<S, LowshelfMode<S>>,
>;

// Highshelf (boost) and HighshelfCut (as you already have)
type HSChain = Pipe<
    Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, DbLin>,
    Svf<S, HighshelfMode<S>>,
>;
type HSChainCut = Pipe<
    Stack<
        Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>,
        Pipe<Stack<Constant<UInt<UTerm, B1>>, DbLin>, super::div::Div<S>>,
    >,
    Svf<S, HighshelfMode<S>>,
>;

// Resonators and allpass leak
type RezChain =
    Pipe<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, Resonator<S, U3>>;
type RezParallel = Pipe<Pipe<Split<U2>, Stack<RezChain, RezChain>>, Join<U2>>;
type RezSerial = Pipe<RezChain, RezChain>;
type APChain =
    Pipe<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, Svf<S, AllpassMode<S>>>;

// Scaled branches used in the variant parallel mix.
type RezScaled = Unop<RezParallel, FrameMulScalar<UInt<UTerm, B1>>>;
type RezSerialScaled = Unop<RezSerial, FrameMulScalar<UInt<UTerm, B1>>>;
type APScaled = Unop<APChain, FrameMulScalar<UInt<UTerm, B1>>>;

// Crossfade
// Crossfade
type CrossFadeChain<C> = Pipe<Stack<Stack<Pass, Pass>, C>, super::crossfade::EqualPowerCrossfade>;
type WetControl =
    Unop<Unop<Control, FrameMulScalar<UInt<UTerm, B1>>>, FrameAddScalar<UInt<UTerm, B1>>>;
// (removed WetControl; using explicit wet/dry MixWetDry later)

// Tilts
type ATiltChain = Pipe<LSChainCut, HSChain>;
type BTiltChain = Pipe<HSChainCut, LSChain>;

// Variant structures (parallel mix shape)
type VariantMix = Pipe<Pipe<Split<U2>, Stack<RezScaled, APScaled>>, Join<U2>>;
type VariantMixB = Pipe<Pipe<Split<U2>, Stack<RezSerialScaled, APScaled>>, Join<U2>>;

// Branch types
type ABranchVariant = Pipe<ATiltChain, VariantMix>;
type ABranchType =
    Pipe<Pipe<Split<U2>, Stack<ABranchVariant, ABranchVariant>>, CrossFadeChain<Control>>;
type BBranchVariant = Pipe<BTiltChain, VariantMixB>;
type BBranchType =
    Pipe<Pipe<Split<U2>, Stack<BBranchVariant, BBranchVariant>>, CrossFadeChain<Control>>;

// Parallel branch filters
type PHigh =
    Pipe<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, Svf<S, HighpassMode<S>>>;

type PLow = Pipe<
    Pipe<
        Pipe<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, Svf<S, LowpassMode<S>>>,
        Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>,
    >,
    Resonator<S, U3>,
>;

type PMid = Pipe<
    Pipe<
        Pipe<
            Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>,
            Svf<S, HighpassMode<S>>,
        >,
        Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>,
    >,
    Svf<S, LowpassMode<S>>,
>;

type PAll =
    Pipe<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, Svf<S, AllpassMode<S>>>;

type LowGain = Unop<
    Binop<
        FrameAdd<UInt<UTerm, B1>>,
        Unop<ClampedControl, FrameMulScalar<UInt<UTerm, B1>>>,
        Unop<
            Unop<
                Pipe<Stack<Control, Constant<UInt<UTerm, B1>>>, super::pow::Pow<S>>,
                FrameNegAddScalar<UInt<UTerm, B1>>,
            >,
            FrameMulScalar<UInt<UTerm, B1>>,
        >,
    >,
    FrameAddScalar<UInt<UTerm, B1>>,
>;

type ParallelLowFilter = Pipe<
    Pipe<Pipe<Split<U3>, Stack<Stack<PHigh, PLow>, PMid>>, Join<U3>>,
    Binop<FrameMul<UInt<UTerm, B1>>, PAll, LowGain>,
>;

// Overall: split -> (A_branch | B_branch) -> crossfade(control_a_b)
pub type FilterType = Pipe<
    Pipe<
        Pipe<Split<U3>, Stack<Stack<ABranchType, BBranchType>, Pass>>,
        Stack<CrossFadeChain<ClampedControl>, ParallelLowFilter>,
    >,
    CrossFadeChain<WetControl>,
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
        >> clip_to(S::EPSILON.sqrt() as f32, (1.0 - S::EPSILON.sqrt()) as f32)
        >> follow::<S>(follow_time as S);
    let control: An<Control> = An(control) >> follow::<S>(follow_time as S);

    // Centers for adjacent (4th and 5th) formants (with branch-specific detune)
    let f4 = (config.formant_hz(4) * (1.0 + config.cents / 1200.0)) as f32;
    let f5 = (config.formant_hz(5) * (1.0 + config.cents / 1200.0)) as f32;

    let a_f4 = f4 * 0.72;
    let a_f5 = f5 * 1.08;
    let b_f4 = f4 * 0.98;
    let b_f5 = f5 * 1.02;

    let pivot_a = (a_f4 + a_f5) * 0.5;
    let pivot_b = (b_f4 + b_f5) * 0.5;

    // A branch
    let a_tilt_lo: An<LSChainCut> = (pass()
        | constant(pivot_a)
        | filter_q_shelf.clone()
        | ((constant(1.0) | filter_shelf_gain_lin.clone()) >> super::div::div::<S>()))
        >> lowshelf::<S>();
    let a_tilt_hi: An<HSChain> =
        (pass() | constant(pivot_a) | filter_q_shelf.clone() | filter_shelf_gain_lin.clone())
            >> highshelf::<S>();
    let a_tilt: An<ATiltChain> = a_tilt_lo >> a_tilt_hi;

    let a_res4: An<RezChain> = (pass() | constant(a_f4) | filter_q_warm.clone()) >> resonator();
    let a_res5: An<RezChain> = (pass() | constant(a_f5) | filter_q_warm.clone()) >> resonator();
    let a_res: An<RezParallel> = split::<U2>() >> (a_res4 | a_res5) >> join::<U2>();
    let a_ap: An<APChain> = (pass() | constant(pivot_a) | filter_q_warm.clone()) >> allpass::<S>();

    let a_variant_wide: An<ABranchVariant> = a_tilt.clone()
        >> (split::<U2>() >> ((a_res * 1.0) | (a_ap.clone() * 0.20)) >> join::<U2>());

    let a_res4_tight: An<RezChain> =
        (pass() | constant(a_f4 * 1.01) | filter_q_bright.clone()) >> resonator();
    let a_res5_tight: An<RezChain> =
        (pass() | constant(a_f5 * 0.99) | filter_q_bright.clone()) >> resonator();
    let a_res_tight: An<RezParallel> =
        split::<U2>() >> (a_res4_tight | a_res5_tight) >> join::<U2>();
    let a_variant_focus: An<ABranchVariant> =
        a_tilt >> (split::<U2>() >> ((a_res_tight * 0.95) | (a_ap * 0.0)) >> join::<U2>());

    let a_inner_xfade: An<CrossFadeChain<Control>> =
        (pass() | pass() | control.clone()) >> super::crossfade::equal_power_crossfade();
    let a_branch: An<ABranchType> =
        split::<U2>() >> (a_variant_wide | a_variant_focus) >> a_inner_xfade;

    // B branch
    let b_tilt_hi: An<HSChainCut> = (pass()
        | constant(pivot_b)
        | filter_q_shelf.clone()
        | ((constant(1.0) | filter_shelf_gain_lin.clone()) >> super::div::div::<S>()))
        >> highshelf::<S>();
    let b_tilt_lo: An<LSChain> =
        (pass() | constant(pivot_b) | filter_q_shelf.clone() | filter_shelf_gain_lin.clone())
            >> lowshelf::<S>();
    let b_tilt: An<BTiltChain> = b_tilt_hi >> b_tilt_lo;

    let b_res4: An<RezChain> = (pass() | constant(b_f4) | filter_q_warm.clone()) >> resonator();
    let b_res5: An<RezChain> = (pass() | constant(b_f5) | filter_q_warm.clone()) >> resonator();
    let b_res_series: An<RezSerial> = b_res4 >> b_res5;
    let b_ap: An<APChain> = (pass() | constant(pivot_b) | filter_q_warm.clone()) >> allpass::<S>();

    let b_variant_wide: An<BBranchVariant> = b_tilt.clone()
        >> (split::<U2>() >> ((b_res_series * 0.90) | (b_ap.clone() * 0.18)) >> join::<U2>());

    let b_res4_bright: An<RezChain> =
        (pass() | constant(b_f4 * 1.01) | filter_q_piercing.clone()) >> resonator();
    let b_res5_bright: An<RezChain> =
        (pass() | constant(b_f5 * 0.99) | filter_q_piercing.clone()) >> resonator();
    let b_res_bright: An<RezSerial> = b_res4_bright >> b_res5_bright;
    let b_variant_focus: An<BBranchVariant> =
        b_tilt >> (split::<U2>() >> ((b_res_bright * 1.0) | (b_ap * 0.0)) >> join::<U2>());

    let b_inner_xfade: An<CrossFadeChain<Control>> =
        (pass() | pass() | control.clone()) >> super::crossfade::equal_power_crossfade();
    let b_branch: An<BBranchType> =
        split::<U2>() >> (b_variant_wide | b_variant_focus) >> b_inner_xfade;

    // Outer A/B crossfade (driven by `control_a_b`)
    let outer_xfade: An<CrossFadeChain<ClampedControl>> =
        (pass() | pass() | control_a_b.clone()) >> super::crossfade::equal_power_crossfade();

    let lb_hp_cut = ((config.formant_hz(1) * 0.05) as f32).max(20.0);
    let lb_lp_cut = ((config.formant_hz(2) * 0.15) as f32).min(180.0);
    let lb_mid_hi = ((config.formant_hz(2) * 12.0) as f32).max(12_000.0);

    // Post-branch low bed
    let lb_hp: An<PHigh> = (pass() | constant(lb_hp_cut) | filter_q_warm.clone()) >> highpass();
    // Resonant low emphasis from filters (organic “self-noise” feel)
    let lb_lp: An<PLow> = (pass() | constant(lb_lp_cut) | filter_q_warm.clone())
        >> lowpass()
        >> (pass() | constant(lb_lp_cut) | filter_q_bright.clone())
        >> resonator();
    //  Global bandwidth framing (keeps lows audible)
    let lb_mp: An<PMid> = (pass() | constant(lb_lp_cut) | filter_q_warm.clone())
        >> highpass()
        >> (pass() | constant(lb_mid_hi) | filter_q_warm.clone())
        >> lowpass();
    // Allpass diffusion (no new content, but more “swim”)
    let lb_ap: An<PAll> = (pass() | constant(lb_lp_cut) | filter_q_warm.clone()) >> allpass();

    let low_gain: An<LowGain> = (control_a_b * 0.25)
        + ((1.0 - ((control.clone() | constant(1.6)) >> super::pow::pow())) * 0.18)
        + 0.8;

    // Tap the filter input (pre A/B), derive a parallel low band, and mix it back post A/B
    let parallel_lb: An<ParallelLowFilter> =
        split::<U3>() >> (lb_hp | lb_lp | lb_mp) >> join::<U3>() >> (lb_ap * low_gain);

    let wet_xfade = (pass() | pass() | (control.clone() * -1.0 + 1.0))
        >> super::crossfade::equal_power_crossfade();

    split::<U3>() >> (a_branch | b_branch | pass()) >> (outer_xfade | parallel_lb) >> wet_xfade
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
