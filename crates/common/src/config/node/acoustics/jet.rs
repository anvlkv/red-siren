use crate::body::materials::Medium;

use super::super::solver::BowlDescriptor;
use super::super::{AcousticLockIn, JetAcousticsInMedium, JetModeStructure, Node, MODAL_EPSILON};
use super::shared;

/// Compute jet-path damping for the chosen jet interaction mode.
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
    (c.structural * 0.7
        + c.medium_viscous * 1.4
        + c.radiation * 0.06
        + c.clapper_coupling * 0.2
        + c.friction_drive * 0.08)
        .clamp(1e-5, 0.95)
}

/// Build acoustics for the single jet interaction mode.
pub(crate) fn acoustics_for_mode(
    node: &Node,
    jet_mode: &JetModeStructure,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> JetAcousticsInMedium {
    let mode_index = jet_mode.source_mode_index;
    let structural_frequency_hz = jet_mode.structural_frequency_hz;
    let (m, _n) = shared::mode_index_pair(mode_index);

    let damping_in_medium = damping_for_medium(
        node,
        mode_index,
        structural_frequency_hz,
        medium,
        descriptor,
    );

    let speed_of_sound_m_per_s = medium.speed_of_sound_m_per_s;
    let acoustic_center_hz =
        (speed_of_sound_m_per_s / (4.0 * descriptor.radius_m.max(MODAL_EPSILON))).max(1.0);
    let frequency_hz = (0.65 * structural_frequency_hz + 0.35 * acoustic_center_hz).max(1.0);

    let ka = 2.0 * std::f64::consts::PI * frequency_hz * descriptor.radius_m
        / speed_of_sound_m_per_s.max(MODAL_EPSILON);
    let radiation_efficiency = shared::radiation_efficiency_from_ka(ka, m);

    let lock_bandwidth_hz = ((0.02 + 0.07 * jet_mode.jet_base.coupling + 0.15 * damping_in_medium)
        * frequency_hz)
        .max(0.5);
    let phase_sensitivity = (1.0 / ((m + 1) as f64).sqrt()).clamp(0.2, 1.0);

    JetAcousticsInMedium {
        source_mode_index: mode_index,
        frequency_hz,
        damping_in_medium,
        acoustic_lock_in: AcousticLockIn {
            lock_center_hz: frequency_hz,
            lock_bandwidth_hz,
            phase_sensitivity,
        },
        radiation_efficiency,
    }
}
