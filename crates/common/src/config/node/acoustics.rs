use nalgebra::{Point3, Vector3};

use crate::body::materials::Medium;

use super::solver::BowlDescriptor;
use super::{Node, MODAL_EPSILON};

pub(super) fn standard_air_medium() -> Medium {
    Medium::standard_air()
}

pub(super) fn mode_index_pair(index: usize) -> (usize, usize) {
    let mut i = index;
    let mut order = 0usize;
    loop {
        for m in 0..=order {
            let n = order + 1 - m;
            if i == 0 {
                return (m, n);
            }
            i -= 1;
        }
        order += 1;
    }
}

pub(super) fn radiation_efficiency_from_ka(ka: f64, circumferential_order: usize) -> f64 {
    let mode_multiplier = (circumferential_order as f64 + 1.0).max(1.0);
    let ka_eff = ka.max(0.0) * mode_multiplier;
    (ka_eff * ka_eff / (1.0 + ka_eff * ka_eff)).clamp(0.0, 1.0)
}

pub(super) fn strike_path_damping_for_medium(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> f64 {
    let (structural, medium_viscous, radiation, clapper_coupling, slide_friction) =
        damping_components_for_medium(node, mode_index, frequency_hz, medium, descriptor);
    (structural
        + medium_viscous
        + radiation * 0.02
        + clapper_coupling * 1.6
        + slide_friction * 0.25)
        .clamp(1e-5, 0.95)
}

pub(super) fn jet_path_damping_for_medium(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> f64 {
    let (structural, medium_viscous, radiation, clapper_coupling, slide_friction) =
        damping_components_for_medium(node, mode_index, frequency_hz, medium, descriptor);
    (structural * 0.7
        + medium_viscous * 1.4
        + radiation * 0.06
        + clapper_coupling * 0.2
        + slide_friction * 0.08)
        .clamp(1e-5, 0.95)
}

pub(super) fn slide_path_damping_for_medium(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> f64 {
    let (structural, medium_viscous, radiation, clapper_coupling, slide_friction) =
        damping_components_for_medium(node, mode_index, frequency_hz, medium, descriptor);
    (structural * 0.5
        + medium_viscous * 1.2
        + radiation * 0.03
        + clapper_coupling * 0.3
        + slide_friction * 2.4)
        .clamp(1e-5, 0.95)
}

pub(super) fn average_coupling(
    vertices: &[Point3<f64>],
    displacement: &[Vector3<f64>],
    min_y: f64,
    y_span: f64,
    target_y: f64,
) -> f64 {
    if vertices.is_empty() || displacement.is_empty() {
        return 0.0;
    }

    let (weighted_sum, weight_sum) =
        vertices
            .iter()
            .zip(displacement.iter())
            .fold((0.0, 0.0), |(sum, wsum), (point, disp)| {
                let y_u = ((point.y - min_y) / y_span).clamp(0.0, 1.0);
                let weight = (1.0 - (y_u - target_y).abs() * 2.5).clamp(0.0, 1.0);
                (sum + disp.norm().abs() * weight, wsum + weight)
            });

    if weight_sum <= MODAL_EPSILON {
        0.0
    } else {
        (weighted_sum / weight_sum).clamp(0.0, 1.0)
    }
}

fn damping_components_for_medium(
    node: &Node,
    mode_index: usize,
    frequency_hz: f64,
    medium: &Medium,
    descriptor: BowlDescriptor,
) -> (f64, f64, f64, f64, f64) {
    let medium_viscosity = medium.viscosity_pa_s;
    let medium_density = medium.density_kg_per_m3;
    let medium_speed_of_sound = medium.speed_of_sound_m_per_s;
    let medium_impedance = medium.impedance_m_rayl;
    let structural = {
        let youngs_modulus_pa = node
            .bowl
            .material
            .youngs_modulus_pa_at_temperature_c(medium.temperature_c);
        let stiffness_factor = (1.0e9 / youngs_modulus_pa.max(1.0e6)).sqrt();
        let poisson_factor = 1.0 + node.bowl.material.poisson_ratio.clamp(-0.49, 0.49).abs() * 0.3;
        let mode_factor = 1.0 + mode_index as f64 * 0.05;
        (0.0012 * stiffness_factor * poisson_factor * mode_factor).max(0.0)
    };

    let medium_viscous = medium_viscosity.max(0.0)
        / (medium_density.max(MODAL_EPSILON)
            * medium_speed_of_sound.max(MODAL_EPSILON)
            * descriptor.thickness_m.max(MODAL_EPSILON));
    let ka = 2.0 * std::f64::consts::PI * frequency_hz * descriptor.radius_m
        / medium_speed_of_sound.max(MODAL_EPSILON);
    let (m, _n) = mode_index_pair(mode_index);
    let radiation = radiation_efficiency_from_ka(ka, m)
        * (medium_impedance.max(0.0)
            / (medium_impedance.max(0.0) + descriptor.areal_density_kg_per_m2.max(1.0)));

    let clapper_coupling = descriptor.clapper_mass_ratio * 0.0025 / ((mode_index + 1) as f64);
    let frequency_factor = (1.0 + (frequency_hz / 1_000.0).clamp(0.0, 2.0) * 0.25).max(0.5);
    let mode_factor = 1.0 + mode_index as f64 * 0.08;
    let slide_friction = node.clapper_to_bowl_friction.max(0.0)
        * descriptor.clapper_mass_ratio.sqrt().max(MODAL_EPSILON)
        * 0.01
        * frequency_factor
        * mode_factor;
    (
        structural,
        medium_viscous,
        radiation,
        clapper_coupling,
        slide_friction,
    )
}
