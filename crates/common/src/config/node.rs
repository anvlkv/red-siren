use nalgebra::{Point3, Vector3};

use crate::body::materials::Medium;
use crate::{
    config::Band, mesh_surface_area_m2, mesh_vertex_normals, nearest_vertex_index, Body, Meshable,
    SurfaceMesh,
};

const MODAL_EPSILON: f64 = 1e-12;
const MIN_MODE_COUNT: usize = 1;
const MOUNT_PROFILE_R_M: f64 = 0.0;
const MOUNT_PROFILE_Y_M: f64 = 0.0;
mod acoustics;
mod builder;
mod shape_profile;
mod solver;
mod types;

pub use builder::*;
pub use shape_profile::*;
pub use types::*;

use solver::per_vertex_thickness;
use solver::{BowlDescriptor, ScalarMode};

impl Node {
    pub fn new(
        bowl: Body<BowlGeometry>,
        clapper: Body<ClapperGeometry>,
        clapper_to_bowl_friction: f64,
    ) -> Self {
        Self {
            bowl,
            clapper,
            clapper_to_bowl_friction,
        }
    }

    pub fn modal_frequencies_hz(&self, resolution: usize, mode_count: usize) -> Vec<f64> {
        let resolution = solver::analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };
        if bowl_mesh.vertex_count() < 3 {
            return vec![];
        }

        let descriptor = match solver::bowl_descriptor(self, resolution, &bowl_mesh, None) {
            Some(desc) => desc,
            None => return vec![],
        };

        let pvt = per_vertex_thickness(
            self,
            resolution,
            bowl_mesh.vertex_count(),
            descriptor.thickness_m,
        );
        solver::solve_scalar_modes(mode_count, &bowl_mesh, descriptor, &pvt)
            .modes
            .into_iter()
            .map(|mode| mode.frequency_hz)
            .collect()
    }

    pub fn mode_shapes(&self, resolution: usize, mode_count: usize) -> Vec<StrikeModeStructure> {
        let resolution = solver::analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };
        self.strike_mode_shapes_from_mesh(mode_count, resolution, &bowl_mesh)
    }

    pub fn modal_participation_factor(
        &self,
        point: Point3<f64>,
        direction: Vector3<f64>,
        resolution: usize,
        mode_count: usize,
    ) -> Vec<f64> {
        let resolution = solver::analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };

        let modes = self.strike_mode_shapes_from_mesh(mode_count, resolution, &bowl_mesh);
        if modes.is_empty() {
            return vec![];
        }

        let dir = match direction.try_normalize(MODAL_EPSILON) {
            Some(d) => d,
            None => return vec![0.0; modes.len()],
        };

        if bowl_mesh.vertices.is_empty() {
            return vec![0.0; modes.len()];
        }

        let nearest_index = nearest_vertex_index(&bowl_mesh.vertices, point);

        modes
            .iter()
            .map(|mode| {
                if mode.vertex_displacement.is_empty() {
                    return 0.0;
                }

                let idx = nearest_index.min(mode.vertex_displacement.len() - 1);
                let projection = mode.vertex_displacement[idx].dot(&dir).abs();
                let max_norm = mode
                    .vertex_displacement
                    .iter()
                    .map(|v| v.norm())
                    .fold(0.0f64, f64::max)
                    .max(MODAL_EPSILON);

                (projection / max_norm).clamp(0.0, 1.0)
            })
            .collect()
    }

    pub fn modal_damping(&self, band: &Band, resolution: usize, mode_count: usize) -> Vec<f64> {
        let resolution = solver::analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };

        if bowl_mesh.vertex_count() < 3 {
            return vec![];
        }

        let descriptor = match solver::bowl_descriptor(
            self,
            resolution,
            &bowl_mesh,
            Some(band.medium.temperature_c),
        ) {
            Some(desc) => desc,
            None => return vec![],
        };

        let pvt = per_vertex_thickness(
            self,
            resolution,
            bowl_mesh.vertex_count(),
            descriptor.thickness_m,
        );
        solver::solve_scalar_modes(mode_count, &bowl_mesh, descriptor, &pvt)
            .modes
            .into_iter()
            .enumerate()
            .map(|(i, mode)| {
                acoustics::strike_path_damping_for_medium(
                    self,
                    i,
                    mode.frequency_hz,
                    &band.medium,
                    descriptor,
                )
            })
            .collect()
    }

    fn bowl_surface_mesh(&self, resolution: usize) -> Option<SurfaceMesh<Point3<f64>, [u32; 3]>> {
        // Use shell mesh (Revolution) for modal solve performance; thick mesh is
        // still used for mass/material computations and deriving per-vertex thickness.
        let mesh = self
            .bowl
            .meshable
            .inner()
            .base
            .base
            .surface_mesh_data(resolution);
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            None
        } else {
            Some(mesh)
        }
    }

    fn strike_mode_shapes_from_mesh(
        &self,
        mode_count: usize,
        resolution: usize,
        bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
    ) -> Vec<StrikeModeStructure> {
        if bowl_mesh.vertex_count() < 3 {
            return vec![];
        }

        let descriptor = match solver::bowl_descriptor(self, resolution, bowl_mesh, None) {
            Some(desc) => desc,
            None => return vec![],
        };

        let pvt = per_vertex_thickness(
            self,
            resolution,
            bowl_mesh.vertex_count(),
            descriptor.thickness_m,
        );
        let solved = solver::solve_scalar_modes(mode_count, bowl_mesh, descriptor, &pvt);
        if solved.modes.is_empty() {
            return vec![];
        }

        self.structural_paths_from_solved_modes(resolution, bowl_mesh, descriptor, &solved.modes)
            .0
    }

    fn structural_paths_from_solved_modes(
        &self,
        resolution: usize,
        bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
        descriptor: BowlDescriptor,
        solved_modes: &[ScalarMode],
    ) -> (
        Vec<StrikeModeStructure>,
        Vec<JetModeStructure>,
        Vec<SlideModeStructure>,
    ) {
        if solved_modes.is_empty() {
            return (vec![], vec![], vec![]);
        }

        let normals = mesh_vertex_normals(&bowl_mesh.vertices, &bowl_mesh.indices, MODAL_EPSILON);
        let (bounds_min, bounds_max) = self.bowl.meshable.bounding_box(resolution);
        let y_span = (bounds_max.y - bounds_min.y).abs().max(MODAL_EPSILON);

        let mut strike_modes = Vec::with_capacity(solved_modes.len());
        let mut jet_modes = Vec::with_capacity(solved_modes.len());
        let mut slide_modes = Vec::with_capacity(solved_modes.len());
        for (i, solved_mode) in solved_modes.iter().enumerate() {
            let frequency_hz = solved_mode.frequency_hz;
            let (m, _n) = acoustics::mode_index_pair(i);
            let displacement = solved_mode
                .amplitudes
                .iter()
                .zip(normals.iter())
                .map(|(amplitude, normal)| *normal * *amplitude)
                .collect::<Vec<_>>();

            let strike_coupling = acoustics::average_coupling(
                &bowl_mesh.vertices,
                &displacement,
                bounds_min.y,
                y_span,
                0.35,
            );
            let jet_coupling = acoustics::average_coupling(
                &bowl_mesh.vertices,
                &displacement,
                bounds_min.y,
                y_span,
                0.9,
            );
            let geometric_slide_coupling = acoustics::average_coupling(
                &bowl_mesh.vertices,
                &displacement,
                bounds_min.y,
                y_span,
                0.62,
            );

            let angle_sensitivity = ((m as f64) / ((m + 2) as f64)).clamp(0.0, 1.0);

            let strouhal_target = (0.17 + 0.015 * (m as f64)).clamp(0.12, 0.42);
            let effective_aperture_m = (descriptor.thickness_m * 2.2)
                .max(descriptor.radius_m * 0.08)
                .max(MODAL_EPSILON);
            let convective_speed_m_per_s = (frequency_hz * effective_aperture_m
                / strouhal_target.max(MODAL_EPSILON))
            .max(MODAL_EPSILON);
            let convective_delay_s =
                (effective_aperture_m / convective_speed_m_per_s).clamp(1e-6, 0.25);
            let threshold_drive = (((1.0 - jet_coupling) * 0.45) + 0.05).clamp(0.0, 2.0);
            let small_signal_gain = (jet_coupling * 1.2 / 0.04).clamp(0.0, 40.0);
            let contact_state = acoustics::slide_contact_state_for_mode(
                self,
                i,
                frequency_hz,
                descriptor,
                geometric_slide_coupling,
            );
            let slide_coupling = (geometric_slide_coupling
                * (0.72 + 0.28 * contact_state.normal_load_proxy)
                + 0.18 * contact_state.slip_drive
                + 0.1 * contact_state.stick_slip_propensity
                - 0.08 * contact_state.contact_intermittency)
                .clamp(0.0, 1.0);
            let roughness_sensitivity = (0.2
                + geometric_slide_coupling * 0.35
                + contact_state.stick_slip_propensity * 0.45)
                .clamp(0.0, 1.0);

            strike_modes.push(StrikeModeStructure {
                mode_index: i,
                vertex_displacement: displacement.clone(),
                strike_base: StrikeStructuralBase {
                    coupling: strike_coupling,
                    angle_sensitivity,
                },
            });
            jet_modes.push(JetModeStructure {
                mode_index: i,
                rim_response: jet_coupling,
                jet_base: JetStructuralBase {
                    coupling: jet_coupling,
                    vortex_dynamics: JetVortexDynamics {
                        strouhal_target,
                        convective_delay_s,
                        threshold_drive,
                        small_signal_gain,
                    },
                },
            });
            slide_modes.push(SlideModeStructure {
                mode_index: i,
                vertex_displacement: displacement,
                slide_base: SlideStructuralBase {
                    coupling: slide_coupling,
                    roughness_sensitivity,
                    contact_state,
                },
            });
        }

        (strike_modes, jet_modes, slide_modes)
    }

    fn compute_strike_acoustics_for_mode(
        &self,
        mode_index: usize,
        frequency_hz: f64,
        medium: &Medium,
        descriptor: BowlDescriptor,
    ) -> StrikeAcousticsInMedium {
        let damping_in_air = acoustics::strike_path_damping_for_medium(
            self,
            mode_index,
            frequency_hz,
            medium,
            descriptor,
        );
        let impact_bandwidth_hz = ((0.01 + damping_in_air * 0.8) * frequency_hz).max(0.0);

        StrikeAcousticsInMedium {
            frequency_hz,
            damping_in_air,
            impact_bandwidth_hz,
        }
    }

    fn compute_jet_acoustics_for_mode(
        &self,
        mode_index: usize,
        structural_frequency_hz: f64,
        medium: &Medium,
        descriptor: BowlDescriptor,
        _strike_base: &StrikeStructuralBase,
        jet_base: &JetStructuralBase,
    ) -> JetAcousticsInMedium {
        let (m, _n) = acoustics::mode_index_pair(mode_index);

        let damping_in_air = acoustics::jet_path_damping_for_medium(
            self,
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
        let radiation_efficiency = acoustics::radiation_efficiency_from_ka(ka, m);

        let lock_bandwidth_hz =
            ((0.02 + 0.07 * jet_base.coupling + 0.15 * damping_in_air) * frequency_hz).max(0.5);
        let phase_sensitivity = (1.0 / ((m + 1) as f64).sqrt()).clamp(0.2, 1.0);

        JetAcousticsInMedium {
            frequency_hz,
            damping_in_air,
            acoustic_lock_in: AcousticLockIn {
                lock_center_hz: frequency_hz,
                lock_bandwidth_hz,
                phase_sensitivity,
            },
            radiation_efficiency,
        }
    }

    fn compute_slide_acoustics_for_mode(
        &self,
        mode_index: usize,
        structural_frequency_hz: f64,
        medium: &Medium,
        descriptor: BowlDescriptor,
        slide_base: &SlideStructuralBase,
    ) -> SlideAcousticsInMedium {
        let slide_frequency_hz = (structural_frequency_hz
            * (1.0 + 0.06 * slide_base.contact_state.slip_drive
                - 0.035 * slide_base.contact_state.contact_intermittency))
            .max(1.0);
        let damping_in_air = acoustics::slide_path_damping_for_medium(
            self,
            mode_index,
            slide_frequency_hz,
            medium,
            descriptor,
        );
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

        SlideAcousticsInMedium {
            frequency_hz: slide_frequency_hz,
            damping_in_air,
            slide_bandwidth_hz,
            squeal_tendency,
            friction_interaction_gain,
        }
    }

    pub fn computed_debug(
        &self,
        resolution: usize,
        mode_count: usize,
        medium: &Medium,
    ) -> Option<NodeComputedDebug> {
        let analysis_resolution = solver::analysis_resolution(resolution);
        let bowl_mesh = self.bowl_surface_mesh(analysis_resolution)?;
        if bowl_mesh.vertex_count() < 3 {
            return None;
        }

        let descriptor = solver::bowl_descriptor(
            self,
            analysis_resolution,
            &bowl_mesh,
            Some(medium.temperature_c),
        )?;
        let pvt = per_vertex_thickness(
            self,
            analysis_resolution,
            bowl_mesh.vertex_count(),
            descriptor.thickness_m,
        );
        let solved = solver::solve_scalar_modes(mode_count, &bowl_mesh, descriptor, &pvt);
        if solved.modes.is_empty() {
            return None;
        }

        let frequencies_hz = solved
            .modes
            .iter()
            .map(|mode| mode.frequency_hz)
            .collect::<Vec<_>>();

        let (strike_modes, jet_modes, slide_modes) = self.structural_paths_from_solved_modes(
            analysis_resolution,
            &bowl_mesh,
            descriptor,
            &solved.modes,
        );

        let strike_damping_in_air = solved
            .modes
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                acoustics::strike_path_damping_for_medium(
                    self,
                    i,
                    mode.frequency_hz,
                    &acoustics::standard_air_medium(),
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let strike_damping_in_medium = solved
            .modes
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                acoustics::strike_path_damping_for_medium(
                    self,
                    i,
                    mode.frequency_hz,
                    medium,
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let jet_damping_in_air = solved
            .modes
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                acoustics::jet_path_damping_for_medium(
                    self,
                    i,
                    mode.frequency_hz,
                    &acoustics::standard_air_medium(),
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let jet_damping_in_medium = solved
            .modes
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                acoustics::jet_path_damping_for_medium(
                    self,
                    i,
                    mode.frequency_hz,
                    medium,
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let slide_damping_in_air = solved
            .modes
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                acoustics::slide_path_damping_for_medium(
                    self,
                    i,
                    mode.frequency_hz,
                    &acoustics::standard_air_medium(),
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let slide_damping_in_medium = solved
            .modes
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                acoustics::slide_path_damping_for_medium(
                    self,
                    i,
                    mode.frequency_hz,
                    medium,
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let mode_acoustics: Vec<ModeInteractionAcoustics> = solved
            .modes
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                let strike = self.compute_strike_acoustics_for_mode(
                    i,
                    mode.frequency_hz,
                    medium,
                    descriptor,
                );
                let jet = self.compute_jet_acoustics_for_mode(
                    i,
                    mode.frequency_hz,
                    medium,
                    descriptor,
                    &strike_modes[i].strike_base,
                    &jet_modes[i].jet_base,
                );
                let slide = self.compute_slide_acoustics_for_mode(
                    i,
                    mode.frequency_hz,
                    medium,
                    descriptor,
                    &slide_modes[i].slide_base,
                );
                ModeInteractionAcoustics { strike, jet, slide }
            })
            .collect();

        let mut path_modes = Vec::with_capacity(solved.modes.len() * 3);
        for ((strike_mode, jet_mode), slide_mode) in strike_modes
            .into_iter()
            .zip(jet_modes.into_iter())
            .zip(slide_modes.into_iter())
        {
            path_modes.push(PathMode::Strike(strike_mode));
            path_modes.push(PathMode::Jet(jet_mode));
            path_modes.push(PathMode::Slide(slide_mode));
        }

        let bowl_surface_area_m2 =
            mesh_surface_area_m2(&bowl_mesh.vertices, &bowl_mesh.indices).max(MODAL_EPSILON);
        let bowl_volume_m3 = self.bowl.meshable.material_volume_m3(analysis_resolution);
        let bowl_mass_kg = self.bowl.mass_kg(analysis_resolution);
        let clapper_mass_kg = self.clapper.mass_kg(analysis_resolution);

        let structure = NodeComputedStructure {
            requested_resolution: resolution,
            analysis_resolution,
            mode_budget: mode_count,
            mesh_vertex_count: bowl_mesh.vertex_count(),
            mesh_triangle_count: bowl_mesh.indices.len(),
            constrained_vertex_count: solved.constrained_count,
            active_vertex_count: solved.active_count,
            bowl_radius_m: descriptor.radius_m,
            bowl_thickness_m: descriptor.thickness_m,
            bowl_areal_density_kg_per_m2: descriptor.areal_density_kg_per_m2,
            bowl_flexural_rigidity: descriptor.flexural_rigidity,
            bowl_surface_area_m2,
            bowl_volume_m3,
            bowl_mass_kg,
            clapper_mass_kg,
            clapper_mass_ratio: descriptor.clapper_mass_ratio,
            solver_total_lumped_mass_kg: solved.diagnostics.total_lumped_mass_kg,
            solver_characteristic_edge_length_m: solved.diagnostics.characteristic_edge_length_m,
            solver_lambda_min_raw: solved.diagnostics.lambda_min_raw,
            solver_lambda_max_raw: solved.diagnostics.lambda_max_raw,
            solver_lambda_min_kept: solved.diagnostics.lambda_min_kept,
            solver_lambda_max_kept: solved.diagnostics.lambda_max_kept,
            solver_dropped_non_finite: solved.diagnostics.dropped_non_finite,
            solver_dropped_non_positive: solved.diagnostics.dropped_non_positive,
            solver_dropped_near_rigid: solved.diagnostics.dropped_near_rigid,
            solver_condition_number: solved.diagnostics.condition_number,
            frequencies_hz: frequencies_hz.clone(),
            path_modes,
        };

        let acoustics = NodeComputedAcoustics {
            frequencies_hz,
            mode_acoustics,
            strike_damping_in_air,
            strike_damping_in_medium,
            jet_damping_in_air,
            jet_damping_in_medium,
            slide_damping_in_air,
            slide_damping_in_medium,
            medium: medium.clone(),
        };

        Some((structure, acoustics))
    }
}

#[cfg(test)]
mod tests;
