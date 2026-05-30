use common::config::{
    AcousticLockIn, JetAcousticsInMedium, JetModeStructure, JetStructuralBase, JetVortexDynamics,
    SlideAcousticsInMedium, SlideContactState, SlideModeStructure, SlideStructuralBase,
    StrikeAcousticsInMedium, StrikeModeStructure, StrikeStructuralBase,
};
use fundsp::prelude::*;

type StrikeComponent<F> = Binop<FrameMul<U1>, Resonator<F, U1>, Constant<U1>>;

/// Build strike response component for one mode.
fn strike_component<F: Real>(
    StrikeAcousticsInMedium {
        mode_index: _,
        frequency_hz,
        damping_in_medium,
        impact_bandwidth_hz,
    }: &StrikeAcousticsInMedium,
    StrikeStructuralBase {
        coupling,
        angle_sensitivity,
    }: &StrikeStructuralBase,
) -> An<StrikeComponent<F>> {
    let coupling = (*coupling).clamp(0.0, 1.0);
    let angle_sensitivity = (*angle_sensitivity).clamp(0.0, 1.0);
    let damping = (*damping_in_medium).clamp(0.0, 0.99);

    // Use structure to warp effective bandwidth (Q) and strike transfer gain.
    let bandwidth_scale = 1.0 + 0.35 * (1.0 - coupling) + 0.2 * (1.0 - angle_sensitivity);
    let effective_bandwidth_hz = (impact_bandwidth_hz * bandwidth_scale).max(0.1);
    let q = (frequency_hz / effective_bandwidth_hz).max(0.01);

    let gain = (1.0 - damping) * (0.05 + coupling).powf(1.1) * (0.05 + angle_sensitivity).powf(0.6);

    resonator_hz::<F>(convert(*frequency_hz), convert(q)) * constant(convert::<f64, f32>(gain))
}

pub type StrikeModalComponent<F, N> = MultiBus<N, StrikeComponent<F>>;

/// Build multi-mode strike response component.
pub fn strike_modal_component<F: Real, N: Size<f32> + Size<StrikeComponent<F>>>(
    modes: &[StrikeAcousticsInMedium],
    mode_structures: &[StrikeModeStructure],
) -> An<StrikeModalComponent<F, N>> {
    debug_assert_eq!(modes.len(), mode_structures.len());

    busi::<N, _, _>(|i| {
        strike_component::<F>(&modes[i as usize], &mode_structures[i as usize].strike_base)
    })
}

type SlideComponent<F> = Binop<FrameMul<U1>, Resonator<F, U1>, Constant<U1>>;

/// Build slide response component for one mode.
fn slide_component<F: Real>(
    SlideAcousticsInMedium {
        mode_index: _,
        frequency_hz,
        damping_in_medium,
        slide_bandwidth_hz,
        squeal_tendency,
        friction_interaction_gain,
    }: &SlideAcousticsInMedium,
    SlideStructuralBase {
        coupling,
        roughness_sensitivity,
        contact_state:
            SlideContactState {
                normal_load_proxy,
                slip_drive,
                stick_slip_propensity,
                contact_intermittency,
            },
    }: &SlideStructuralBase,
) -> An<SlideComponent<F>> {
    let coupling = (*coupling).clamp(0.0, 1.0);
    let roughness = (*roughness_sensitivity).clamp(0.0, 1.0);
    let normal_load = (*normal_load_proxy).clamp(0.0, 1.0);
    let slip = (*slip_drive).clamp(0.0, 1.0);
    let stick_slip = (*stick_slip_propensity).clamp(0.0, 1.0);
    let intermittency = (*contact_intermittency).clamp(0.0, 1.0);

    let damping = (*damping_in_medium).clamp(0.0, 0.99);
    let squeal = (*squeal_tendency).clamp(0.0, 1.0);
    let friction_gain = (*friction_interaction_gain).clamp(0.0, 1.0);

    // Structural + contact terms warp effective bandwidth (Q).
    let bandwidth_scale = 1.0
        + 0.20 * (1.0 - coupling)
        + 0.10 * (1.0 - roughness)
        + 0.20 * slip
        + 0.25 * intermittency;
    let effective_bandwidth_hz = (slide_bandwidth_hz * bandwidth_scale).max(0.1);
    let q = (frequency_hz / effective_bandwidth_hz).max(0.01);

    // Structural/contact-weighted transfer gain.
    let gain = (0.2 + 0.5 * friction_gain + 0.3 * squeal)
        * (1.0 - damping)
        * (0.45 + 0.55 * coupling)
        * (0.75 + 0.25 * roughness)
        * (0.80 + 0.20 * normal_load)
        * (0.75 + 0.25 * stick_slip)
        * (1.0 - 0.30 * intermittency);

    resonator_hz::<F>(convert(*frequency_hz), convert(q)) * constant(convert::<f64, f32>(gain))
}

pub type SlideModalComponent<F, N> =
    Pipe<Binop<FrameMul<U1>, Noise, Pass>, MultiBus<N, SlideComponent<F>>>;
type JetLoopFilter = Binop<FrameMul<U1>, FixedSvf<f32, LowpassMode<f32>>, Constant<U1>>;
type JetFeedbackLoop = Pipe<Pipe<Delay, JetLoopFilter>, Shaper<Tanh>>;
type JetFeedback = Feedback<U1, JetFeedbackLoop, FrameId<U1>>;
type JetResonatorGain<F> = Binop<FrameMul<U1>, Resonator<F, U1>, Constant<U1>>;
type JetModalComponent<F> = Pipe<JetFeedback, JetResonatorGain<F>>;

/// Build multi-mode slide response component.
pub fn slide_modal_component<F: Real, N: Size<f32> + Size<SlideComponent<F>>>(
    modes: &[SlideAcousticsInMedium],
    mode_structures: &[SlideModeStructure],
) -> An<SlideModalComponent<F, N>> {
    debug_assert_eq!(modes.len(), mode_structures.len());

    (noise() * pass())
        >> busi::<N, _, _>(|i| {
            slide_component::<F>(&modes[i as usize], &mode_structures[i as usize].slide_base)
        })
}

/// Build single-mode jet lock-in response component for one node.
pub fn jet_modal_component<F: Real>(
    JetAcousticsInMedium {
        source_mode_index: _,
        frequency_hz,
        damping_in_medium,
        acoustic_lock_in:
            AcousticLockIn {
                lock_center_hz,
                lock_bandwidth_hz,
                phase_sensitivity,
            },
        radiation_efficiency,
    }: &JetAcousticsInMedium,
    jet_mode: &JetModeStructure,
) -> An<JetModalComponent<F>> {
    let JetStructuralBase {
        coupling,
        vortex_dynamics:
            JetVortexDynamics {
                strouhal_target,
                convective_delay_s,
                threshold_drive,
                small_signal_gain,
            },
    } = &jet_mode.jet_base;

    let damping = (*damping_in_medium).clamp(0.0, 0.99);
    let phase = (*phase_sensitivity).clamp(0.0, 1.0);
    let radiation = (*radiation_efficiency).clamp(0.0, 1.0);

    let coupling = (*coupling).clamp(0.0, 1.0);
    let strouhal_target = (*strouhal_target).clamp(0.05, 4.0);
    let convective_delay_s = (*convective_delay_s).clamp(0.0, 0.5);
    let threshold_drive = (*threshold_drive).clamp(0.0, 1.0);
    let small_signal_gain = (*small_signal_gain).clamp(0.0, 2.0);

    let bandwidth_hz = (*lock_bandwidth_hz).max(0.1);

    // Pull effective lock center between acoustic lock and vortex preference.
    let vortex_pref_hz = (frequency_hz * strouhal_target).max(1.0);
    let lock_pull = 0.15 + 0.70 * coupling;
    let lock_center_eff_hz =
        ((1.0 - lock_pull) * *lock_center_hz + lock_pull * vortex_pref_hz).max(1.0);

    // Convective phase decides how reinforcing the feedback branch is.
    let convective_phase = 2.0 * std::f64::consts::PI * lock_center_eff_hz * convective_delay_s;
    let reinforcement = (0.5 * (1.0 + convective_phase.cos())).clamp(0.0, 1.0);

    let onset_bias = (0.5 * (1.0 + (6.0 * (0.5 - threshold_drive)).tanh())).clamp(0.0, 1.0);

    let effective_bandwidth_hz = (bandwidth_hz * (1.0 + 0.6 * (1.0 - reinforcement))).max(0.1);
    let q = (lock_center_eff_hz / effective_bandwidth_hz).max(0.01);

    let loop_gain = (small_signal_gain
        * (0.30 + 0.70 * coupling)
        * (0.25 + 0.75 * phase)
        * (0.20 + 0.80 * onset_bias)
        * (0.10 + 0.90 * reinforcement)
        * (1.0 - damping))
        .clamp(0.0, 0.95);

    let output_gain = ((0.20 + 0.80 * radiation)
        * (0.35 + 0.65 * coupling)
        * (0.30 + 0.70 * onset_bias)
        * (1.0 - 0.40 * damping))
        .clamp(0.0, 1.25);

    feedback::<U1, _>(
        delay(convective_delay_s)
            >> lowpass_hz(
                convert::<f64, f32>((1.5 * lock_center_eff_hz).max(30.0)),
                convert(0.707),
            ) * constant(convert::<f64, f32>(loop_gain))
            >> shape(Tanh(1.0)),
    ) >> (resonator_hz::<F>(convert(lock_center_eff_hz), convert(q))
        * constant(convert::<f64, f32>(output_gain)))
}
