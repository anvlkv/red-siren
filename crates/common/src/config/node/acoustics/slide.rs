use crate::body::materials::Medium;

use super::super::solver::BowlDescriptor;
use super::super::{
    Node, SlideAcousticsInMedium, SlideContactState, SlideModeStructure, MODAL_EPSILON,
};
use super::shared;

/// Derive slide contact-state metrics from node and mode context.
///
/// All outputs are unitless in [0, 1].
pub(crate) fn contact_state_for_mode(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    descriptor: BowlDescriptor,
    geometric_slide_coupling: f64,
) -> SlideContactState {
    let friction = node.clapper_to_bowl_friction.max(0.0);
    let mode_scale = 1.0 + mode_index as f64 * 0.08;
    let mode_weight = 1.0 / mode_scale.sqrt();
    let frequency_weight = (0.7 + 0.3 * (frequency_hz / 900.0).clamp(0.0, 1.5)).clamp(0.5, 1.3);

    let normal_load_proxy = (descriptor.clapper_mass_ratio.sqrt()
        * (0.35 + 0.65 * geometric_slide_coupling)
        * mode_weight)
        .clamp(0.0, 1.0);

    let slip_drive = (friction
        * normal_load_proxy
        * frequency_weight
        * (0.75 + 0.25 * geometric_slide_coupling)
        * mode_scale.powf(0.2))
    .clamp(0.0, 1.0);

    let stick_slip_propensity =
        (slip_drive * (0.6 + 0.4 * (1.0 - geometric_slide_coupling)) * (0.9 + 0.1 * mode_scale))
            .clamp(0.0, 1.0);

    let contact_intermittency =
        (0.2 + friction * 0.45 + (1.0 - normal_load_proxy) * 0.25 + mode_index as f64 * 0.015)
            .clamp(0.0, 1.0);

    SlideContactState {
        normal_load_proxy,
        slip_drive,
        stick_slip_propensity,
        contact_intermittency,
    }
}

/// Compute slide-path damping for one slide mode.
///
/// Returns damping in [1e-5, 0.95].
pub(crate) fn damping_for_medium(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> f64 {
    let c =
        shared::damping_components_for_medium(node, mode_index, frequency_hz, medium, descriptor);
    (c.structural * 0.5
        + c.medium_viscous * 1.2
        + c.radiation * 0.03
        + c.clapper_coupling * 0.3
        + c.friction_drive * 2.4)
        .clamp(1e-5, 0.95)
}

/// Build slide acoustics for one slide mode.
pub(crate) fn acoustics_for_mode(
    node: &Node,
    slide_mode: &SlideModeStructure,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> SlideAcousticsInMedium {
    let structural_frequency_hz = slide_mode.frequency_hz;
    let mode_index = slide_mode.mode_index;
    let slide_base = &slide_mode.slide_base;

    let slide_frequency_hz = (structural_frequency_hz
        * (1.0 + 0.06 * slide_base.contact_state.slip_drive
            - 0.035 * slide_base.contact_state.contact_intermittency))
        .max(1.0);
    let damping_in_air =
        damping_for_medium(node, mode_index, slide_frequency_hz, medium, descriptor);
    let friction_interaction_gain = (0.25 * slide_base.contact_state.normal_load_proxy
        + 0.35 * slide_base.contact_state.slip_drive
        + 0.25 * slide_base.contact_state.stick_slip_propensity
        + 0.15 * (1.0 - slide_base.contact_state.contact_intermittency))
        .clamp(0.0, 1.0);
    let slide_bandwidth_hz = ((0.01
        + 0.2 * slide_base.coupling
        + 0.45 * friction_interaction_gain
        + damping_in_air * 0.45)
        * slide_frequency_hz)
        .max(0.2);
    let squeal_tendency = ((slide_base.roughness_sensitivity
        * (0.55 + 0.45 * slide_base.contact_state.stick_slip_propensity)
        * (1.0 - damping_in_air)
        * (0.7 + 0.3 * friction_interaction_gain))
        - 0.2 * slide_base.contact_state.contact_intermittency)
        .clamp(0.0, 1.0);

    let _ = MODAL_EPSILON;

    SlideAcousticsInMedium {
        mode_index,
        frequency_hz: slide_frequency_hz,
        damping_in_air,
        slide_bandwidth_hz,
        squeal_tendency,
        friction_interaction_gain,
    }
}
