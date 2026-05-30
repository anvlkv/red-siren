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

use acoustics::{jet, shared, slide, strike};
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
                strike::damping_for_medium(self, i, mode.frequency_hz, &band.medium, descriptor)
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
        JetModeStructure,
        Vec<SlideModeStructure>,
    ) {
        if solved_modes.is_empty() {
            return (
                vec![],
                JetModeStructure {
                    source_mode_index: 0,
                    structural_frequency_hz: 0.0,
                    rim_response: 0.0,
                    jet_base: JetStructuralBase {
                        coupling: 0.0,
                        vortex_dynamics: JetVortexDynamics {
                            strouhal_target: 0.0,
                            convective_delay_s: 0.0,
                            threshold_drive: 0.0,
                            small_signal_gain: 0.0,
                        },
                    },
                },
                vec![],
            );
        }

        let normals = mesh_vertex_normals(&bowl_mesh.vertices, &bowl_mesh.indices, MODAL_EPSILON);
        let (bounds_min, bounds_max) = self.bowl.meshable.bounding_box(resolution);
        let y_span = (bounds_max.y - bounds_min.y).abs().max(MODAL_EPSILON);

        let mut strike_modes = Vec::with_capacity(solved_modes.len());
        let mut best_jet_mode: Option<JetModeStructure> = None;
        let mut slide_modes = Vec::with_capacity(solved_modes.len());
        for (i, solved_mode) in solved_modes.iter().enumerate() {
            let frequency_hz = solved_mode.frequency_hz;
            let (m, _n) = shared::mode_index_pair(i);
            let displacement = solved_mode
                .amplitudes
                .iter()
                .zip(normals.iter())
                .map(|(amplitude, normal)| *normal * *amplitude)
                .collect::<Vec<_>>();

            let strike_coupling = shared::average_coupling(
                &bowl_mesh.vertices,
                &displacement,
                bounds_min.y,
                y_span,
                0.35,
            );
            let jet_coupling = shared::average_coupling(
                &bowl_mesh.vertices,
                &displacement,
                bounds_min.y,
                y_span,
                0.9,
            );
            let geometric_slide_coupling = shared::average_coupling(
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
            let contact_state = slide::contact_state_for_mode(
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
                frequency_hz,
                vertex_displacement: displacement.clone(),
                strike_base: StrikeStructuralBase {
                    coupling: strike_coupling,
                    angle_sensitivity,
                },
            });
            let jet_mode = JetModeStructure {
                source_mode_index: i,
                structural_frequency_hz: frequency_hz,
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
            };
            match best_jet_mode {
                Some(current) if current.rim_response >= jet_mode.rim_response => {}
                _ => best_jet_mode = Some(jet_mode),
            }
            slide_modes.push(SlideModeStructure {
                mode_index: i,
                frequency_hz,
                vertex_displacement: displacement,
                slide_base: SlideStructuralBase {
                    coupling: slide_coupling,
                    roughness_sensitivity,
                    contact_state,
                },
            });
        }

        (
            strike_modes,
            best_jet_mode.expect("at least one solved mode should produce a jet mode"),
            slide_modes,
        )
    }

    pub fn computed_structure(
        &self,
        resolution: usize,
        mode_count: usize,
    ) -> Option<NodeComputedStructure> {
        let analysis_resolution = solver::analysis_resolution(resolution);
        let bowl_mesh = self.bowl_surface_mesh(analysis_resolution)?;
        if bowl_mesh.vertex_count() < 3 {
            return None;
        }

        let descriptor = solver::bowl_descriptor(self, analysis_resolution, &bowl_mesh, None)?;
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

        let (strike_modes, jet_mode, slide_modes) = self.structural_paths_from_solved_modes(
            analysis_resolution,
            &bowl_mesh,
            descriptor,
            &solved.modes,
        );

        let bowl_surface_area_m2 =
            mesh_surface_area_m2(&bowl_mesh.vertices, &bowl_mesh.indices).max(MODAL_EPSILON);
        let bowl_volume_m3 = self.bowl.meshable.material_volume_m3(analysis_resolution);
        let bowl_mass_kg = self.bowl.mass_kg(analysis_resolution);
        let clapper_mass_kg = self.clapper.mass_kg(analysis_resolution);

        Some(NodeComputedStructure {
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
            strike_modes,
            jet_mode,
            slide_modes,
        })
    }

    pub fn computed_acoustics_from_structure(
        &self,
        structure: &NodeComputedStructure,
        medium: &Medium,
    ) -> Option<NodeComputedAcoustics> {
        let bowl_mesh = self.bowl_surface_mesh(structure.analysis_resolution)?;
        if bowl_mesh.vertex_count() < 3 {
            return None;
        }

        let descriptor = solver::bowl_descriptor(
            self,
            structure.analysis_resolution,
            &bowl_mesh,
            Some(medium.temperature_c),
        )?;

        let strike_acoustics = structure
            .strike_modes
            .iter()
            .map(|mode| {
                strike::acoustics_for_mode(
                    self,
                    mode.mode_index,
                    mode.frequency_hz,
                    medium,
                    descriptor,
                )
            })
            .collect::<Vec<_>>();
        let jet_acoustics = jet::acoustics_for_mode(self, &structure.jet_mode, medium, descriptor);
        let slide_acoustics = structure
            .slide_modes
            .iter()
            .map(|mode| slide::acoustics_for_mode(self, mode, medium, descriptor))
            .collect::<Vec<_>>();

        Some(NodeComputedAcoustics {
            strike_modes: strike_acoustics,
            jet_mode: jet_acoustics,
            slide_modes: slide_acoustics,
            medium: medium.clone(),
        })
    }

    pub fn computed_debug(
        &self,
        resolution: usize,
        mode_count: usize,
        medium: &Medium,
    ) -> Option<NodeComputedDebug> {
        let structure = self.computed_structure(resolution, mode_count)?;
        let acoustics = self.computed_acoustics_from_structure(&structure, medium)?;

        Some((structure, acoustics))
    }
}

#[cfg(test)]
mod tests;
