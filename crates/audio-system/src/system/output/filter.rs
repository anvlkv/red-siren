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

type ClampedControl = Pipe<Pipe<Var, Shaper<ClipTo>>, Follow<S>>;

// Bandpass node with constant center frequency and finetuned Q.
type BPChain =
    Pipe<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, Svf<S, BandpassMode<S>>>;

// Highshelf node with constant cutoff, finetuned Q, and finetuned gain.
type HSChain = Pipe<
    Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, FineTunedValue>,
    Svf<S, HighshelfMode<S>>,
>;

// Highshelf node with constant cutoff, finetuned Q, and computed reciprocal gain (for cuts)
type HSChainCut = Pipe<
    Stack<
        Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>,
        Pipe<Stack<Constant<UInt<UTerm, B1>>, FineTunedValue>, super::div::Div<S>>,
    >,
    Svf<S, HighshelfMode<S>>,
>;

// Two bandpasses in series (F4 -> F5)
type ResonatorSerial = Pipe<BPChain, BPChain>;

// Two bandpasses in parallel mixed back
type ResonatorParallel = Pipe<Pipe<Split<U2>, Stack<BPChain, BPChain>>, Join<U2>>;

// Equal-power crossfade: (x, y, control) -> mix
type CrossFadeChain<C> = Pipe<Stack<Stack<Pass, Pass>, C>, super::crossfade::EqualPowerCrossfade>;

// A-branch structure: HS boost pre, then parallel BP (BP4 | BP5)
type ABranchVariant = Pipe<HSChain, ResonatorParallel>;
type ABranchType =
    Pipe<Pipe<Split<U2>, Stack<ABranchVariant, ABranchVariant>>, CrossFadeChain<Control>>;

// B-branch structure: ((BP4 >> BP5) >> HS cut) for both variants (earthly, alien)
type BBranchVariant = Pipe<ResonatorSerial, HSChainCut>;
type BBranchType =
    Pipe<Pipe<Split<U2>, Stack<BBranchVariant, BBranchVariant>>, CrossFadeChain<Control>>;

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
        filter_shelf_gain_lin,
        filter_q_warm,
        ..
    } = finetuned_values.clone();

    #[cfg(feature = "editor")]
    let follow_time = filter_morph_follow_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = filter_morph_follow_s.value()[0];

    let control_a_b: An<ClampedControl> = An(control_a_b)
        >> clip_to(S::EPSILON.sqrt() as f32, (1.0 - S::EPSILON.sqrt()) as f32)
        >> follow::<S>(follow_time as S);
    let control: An<Control> = An(control) >> follow::<S>(follow_time as S);

    // Centers for adjacent (4th and 5th) formants (with branch-specific detune)
    let f4 = (config.formant_hz(4) * (1.0 + config.cents / 1200.0)) as f32;
    let f5 = (config.formant_hz(5) * (1.0 + config.cents / 1200.0)) as f32;

    let a_f4 = f4 * 0.92;
    let a_f5 = f5 * 1.08;
    let b_f4 = f4 * 0.98;
    let b_f5 = f5 * 1.02;

    let fc_shelf_a = (a_f4 + a_f5) / 2.0;
    let fc_shelf_b = (b_f4 + b_f5) / 2.0;

    // A branch — Crystal: HS before, then two BP with pass Q
    let a_crystal_hs: An<HSChain> =
        (pass() | constant(fc_shelf_a) | filter_q_shelf.clone() | filter_shelf_gain_lin.clone())
            >> highshelf::<S>();
    let a_crystal_bp4: An<BPChain> =
        (pass() | constant(a_f4) | filter_q_warm.clone()) >> bandpass::<S>();
    let a_crystal_bp5: An<BPChain> =
        (pass() | constant(a_f5) | filter_q_warm.clone()) >> bandpass::<S>();
    let a_crystal: An<ABranchVariant> =
        a_crystal_hs >> (split::<U2>() >> (a_crystal_bp4 | a_crystal_bp5) >> join::<U2>());

    // A branch — Metal: HS before, then two BP with moog Q
    let a_metal_hs: An<HSChain> =
        (pass() | constant(fc_shelf_a) | filter_q_shelf.clone() | filter_shelf_gain_lin.clone())
            >> highshelf::<S>();
    let a_metal_bp4: An<BPChain> =
        (pass() | constant(a_f4) | filter_q_bright.clone()) >> bandpass::<S>();
    let a_metal_bp5: An<BPChain> =
        (pass() | constant(a_f5) | filter_q_bright.clone()) >> bandpass::<S>();
    let a_metal: An<ABranchVariant> =
        a_metal_hs >> (split::<U2>() >> (a_metal_bp4 | a_metal_bp5) >> join::<U2>());

    let a_inner_xfade: An<CrossFadeChain<Control>> =
        (pass() | pass() | control.clone()) >> super::crossfade::equal_power_crossfade();
    let a_branch: An<ABranchType> = split::<U2>() >> (a_crystal | a_metal) >> a_inner_xfade;

    // B branch — Earthly: two BP with pass Q, then HS after
    let b_earthly_bp4: An<BPChain> =
        (pass() | constant(b_f4) | filter_q_warm.clone()) >> bandpass::<S>();
    let b_earthly_bp5: An<BPChain> =
        (pass() | constant(b_f5) | filter_q_warm.clone()) >> bandpass::<S>();
    let b_earthly_hs: An<HSChainCut> = (pass()
        | constant(fc_shelf_b)
        | filter_q_shelf.clone()
        | ((constant(1.0) | filter_shelf_gain_lin.clone()) >> super::div::div::<S>()))
        >> highshelf::<S>();
    let b_earthly: An<BBranchVariant> = (b_earthly_bp4 >> b_earthly_bp5) >> b_earthly_hs;

    // B branch — Alien: two BP with allpass Q, then HS after
    let b_alien_bp4: An<BPChain> =
        (pass() | constant(b_f4) | filter_q_piercing.clone()) >> bandpass::<S>();
    let b_alien_bp5: An<BPChain> =
        (pass() | constant(b_f5) | filter_q_piercing.clone()) >> bandpass::<S>();
    let b_alien_hs: An<HSChainCut> = (pass()
        | constant(fc_shelf_b)
        | filter_q_shelf
        | ((constant(1.0) | filter_shelf_gain_lin) >> super::div::div::<S>()))
        >> highshelf::<S>();
    let b_alien: An<BBranchVariant> = (b_alien_bp4 >> b_alien_bp5) >> b_alien_hs;

    let b_inner_xfade: An<CrossFadeChain<Control>> =
        (pass() | pass() | control.clone()) >> super::crossfade::equal_power_crossfade();
    let b_branch: An<BBranchType> = split::<U2>() >> (b_earthly | b_alien) >> b_inner_xfade;

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
        + ((1.0 - ((control | constant(1.6)) >> super::pow::pow())) * 0.18)
        + 0.8;

    // Tap the filter input (pre A/B), derive a parallel low band, and mix it back post A/B
    let parallel_lb: An<ParallelLowFilter> =
        split::<U3>() >> (lb_hp | lb_lp | lb_mp) >> join::<U3>() >> (lb_ap * low_gain);

    split::<U3>() >> (a_branch | b_branch | pass()) >> (outer_xfade | parallel_lb) >> join::<U2>()
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
