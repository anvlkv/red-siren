use crate::body::materials::Medium;

use super::super::solver::BowlDescriptor;
use super::super::{Node, StrikeAcousticsInMedium};
use super::shared;

/// Compute strike-path damping for one modal interaction.
///
/// Returns damping in [1e-5, 0.95].
pub(crate) fn damping_for_medium(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> f64 {
    let c = shared::damping_components_for_medium(node, mode_index, frequency_hz, medium, descriptor);
    (c.structural + c.medium_viscous + c.radiation * 0.02 + c.clapper_coupling * 1.6 + c.friction_drive * 0.25)
        .clamp(1e-5, 0.95)
}

/// Build strike acoustics for one interaction mode.
pub(crate) fn acoustics_for_mode(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> StrikeAcousticsInMedium {
    let damping_in_air = damping_for_medium(node, mode_index, frequency_hz, medium, descriptor);
    let impact_bandwidth_hz = ((0.01 + damping_in_air * 0.8) * frequency_hz).max(0.0);

    StrikeAcousticsInMedium {
        mode_index,
        frequency_hz,
        damping_in_air,
        impact_bandwidth_hz,
    }
}
