use nalgebra::{DMatrix, Point3, Vector3};
use serde::{Deserialize, Serialize};

use crate::body::materials::Medium;
use crate::{
    config::Band, mesh_surface_area_m2, mesh_vertex_normals, nearest_vertex_index, Body, Meshable,
    RevolutionMesh, SurfaceMesh, ThickMesh,
};

const MODAL_EPSILON: f64 = 1e-12;
const MIN_MODE_COUNT: usize = 1;
const MAX_MODE_COUNT: usize = 24;
const MOUNT_PROFILE_R_M: f64 = 0.0;
const MOUNT_PROFILE_Y_M: f64 = 0.0;
/// Hard cap on the mesh resolution used for the FEM eigen solve.
/// At resolution > ~12 the active-DOF count grows quadratically and
/// the dense O(N³) symmetric_eigen becomes intractable in debug builds.
const MODAL_EIGEN_MAX_RESOLUTION: usize = 8;

#[derive(Clone, Copy, Debug)]
struct BowlDescriptor {
    radius_m: f64,
    thickness_m: f64,
    areal_density_kg_per_m2: f64,
    flexural_rigidity: f64,
    clapper_mass_ratio: f64,
}

#[derive(Clone, Debug)]
struct ScalarMode {
    frequency_hz: f64,
    amplitudes: Vec<f64>,
}

fn standard_air_medium() -> Medium {
    Medium {
        density_kg_per_m3: 1.225,
        speed_of_sound_m_per_s: 343.0,
        viscosity_pa_s: 1.8e-5,
        impedance_m_rayl: 420.0,
    }
}

fn triangle_area(points: &[Point3<f64>], tri: [u32; 3]) -> Option<f64> {
    let [a, b, c] = tri;
    let (a, b, c) = (a as usize, b as usize, c as usize);
    if a >= points.len() || b >= points.len() || c >= points.len() {
        return None;
    }

    let pa = points[a];
    let pb = points[b];
    let pc = points[c];
    let area = 0.5 * (pb - pa).cross(&(pc - pa)).norm();
    if area.is_finite() && area > MODAL_EPSILON {
        Some(area)
    } else {
        None
    }
}

fn cotangent(a: Point3<f64>, b: Point3<f64>, c: Point3<f64>) -> Option<f64> {
    let u = b - a;
    let v = c - a;
    let cross_norm = u.cross(&v).norm();
    if !cross_norm.is_finite() || cross_norm <= MODAL_EPSILON {
        return None;
    }

    let dot = u.dot(&v);
    let cot = dot / cross_norm;
    if cot.is_finite() {
        Some(cot)
    } else {
        None
    }
}

fn accumulate_symmetric_weight(matrix: &mut DMatrix<f64>, i: usize, j: usize, weight: f64) {
    if !weight.is_finite() || weight <= 0.0 || i == j {
        return;
    }

    matrix[(i, i)] += weight;
    matrix[(j, j)] += weight;
    matrix[(i, j)] -= weight;
    matrix[(j, i)] -= weight;
}

fn mode_index_pair(index: usize) -> (usize, usize) {
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

pub type BowlGeometry = ThickMesh<RevolutionMesh<5>>;

pub type ClapperGeometry = RevolutionMesh<3>;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Node {
    pub bowl: Body<BowlGeometry>,
    pub clapper: Body<ClapperGeometry>,
}

impl Node {
    pub fn new(bowl: Body<BowlGeometry>, clapper: Body<ClapperGeometry>) -> Self {
        Self { bowl, clapper }
    }

    fn analysis_resolution(&self, resolution: usize) -> usize {
        let requested = resolution.max(8);
        let optimal = self.bowl.meshable.opt_resolution().max(8);
        requested.min(optimal).min(MODAL_EIGEN_MAX_RESOLUTION)
    }

    pub fn modal_frequencies_hz(&self, resolution: usize) -> Vec<f64> {
        let mode_count = self.mode_budget(resolution);
        let resolution = self.analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };
        if bowl_mesh.vertex_count() < 3 {
            return vec![];
        }

        let descriptor = match self.bowl_descriptor(resolution, &bowl_mesh) {
            Some(desc) => desc,
            None => return vec![],
        };

        self.solve_scalar_modes(mode_count, &bowl_mesh, descriptor)
            .into_iter()
            .map(|mode| mode.frequency_hz)
            .collect()
    }

    pub fn mode_shapes(&self, resolution: usize) -> Vec<ModeShape> {
        let mode_count = self.mode_budget(resolution);
        let resolution = self.analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };
        self.mode_shapes_from_mesh(mode_count, resolution, &bowl_mesh)
    }

    pub fn modal_participation_factor(
        &self,
        point: Point3<f64>,
        direction: Vector3<f64>,
        resolution: usize,
    ) -> Vec<f64> {
        let mode_count = self.mode_budget(resolution);
        let resolution = self.analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };

        let modes = self.mode_shapes_from_mesh(mode_count, resolution, &bowl_mesh);
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

    pub fn modal_damping(&self, band: &Band, resolution: usize) -> Vec<f64> {
        let mode_count = self.mode_budget(resolution);
        let resolution = self.analysis_resolution(resolution);
        let bowl_mesh = match self.bowl_surface_mesh(resolution) {
            Some(mesh) => mesh,
            None => return vec![],
        };

        if bowl_mesh.vertex_count() < 3 {
            return vec![];
        }

        let descriptor = match self.bowl_descriptor(resolution, &bowl_mesh) {
            Some(desc) => desc,
            None => return vec![],
        };

        self.solve_scalar_modes(mode_count, &bowl_mesh, descriptor)
            .into_iter()
            .enumerate()
            .map(|(i, mode)| {
                self.mode_damping_for_medium(i, mode.frequency_hz, &band.medium, descriptor)
            })
            .collect()
    }

    fn bowl_surface_mesh(&self, resolution: usize) -> Option<SurfaceMesh<Point3<f64>, [u32; 3]>> {
        let mesh = self.bowl.meshable.surface_mesh_data(resolution);
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            None
        } else {
            Some(mesh)
        }
    }

    fn mode_shapes_from_mesh(
        &self,
        mode_count: usize,
        resolution: usize,
        bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
    ) -> Vec<ModeShape> {
        if bowl_mesh.vertex_count() < 3 {
            return vec![];
        }

        let descriptor = match self.bowl_descriptor(resolution, bowl_mesh) {
            Some(desc) => desc,
            None => return vec![],
        };

        let solved_modes = self.solve_scalar_modes(mode_count, bowl_mesh, descriptor);
        if solved_modes.is_empty() {
            return vec![];
        }

        let normals = mesh_vertex_normals(&bowl_mesh.vertices, &bowl_mesh.indices, MODAL_EPSILON);
        let (bounds_min, bounds_max) = self.bowl.meshable.bounding_box(resolution);
        let y_span = (bounds_max.y - bounds_min.y).abs().max(MODAL_EPSILON);
        let air = standard_air_medium();

        let mut modes = Vec::with_capacity(solved_modes.len());
        for (i, solved_mode) in solved_modes.iter().enumerate() {
            let frequency_hz = solved_mode.frequency_hz;
            let (m, _n) = mode_index_pair(i);
            let displacement = solved_mode
                .amplitudes
                .iter()
                .zip(normals.iter())
                .map(|(amplitude, normal)| *normal * *amplitude)
                .collect::<Vec<_>>();

            let angle_sensitivity = ((m as f64) / ((m + 2) as f64)).clamp(0.0, 1.0);
            let strike_coupling = average_coupling(
                &bowl_mesh.vertices,
                &displacement,
                bounds_min.y,
                y_span,
                0.35,
            );
            let jet_coupling = average_coupling(
                &bowl_mesh.vertices,
                &displacement,
                bounds_min.y,
                y_span,
                0.9,
            );
            let ka = 2.0 * std::f64::consts::PI * frequency_hz * descriptor.radius_m
                / air.speed_of_sound_m_per_s.max(MODAL_EPSILON);
            let radiation_efficiency = (ka * ka / (1.0 + ka * ka)).clamp(0.0, 1.0);
            let damping = self.mode_damping_for_medium(i, frequency_hz, &air, descriptor);

            modes.push(ModeShape {
                vertex_displacement: displacement,
                frequency_hz,
                lock_in_range_hz: (0.015 + damping) * frequency_hz,
                angle_sensitivity,
                strike_coupling,
                jet_coupling,
                radiation_efficiency,
                damping,
            });
        }

        modes
    }

    fn solve_scalar_modes(
        &self,
        mode_count: usize,
        bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
        descriptor: BowlDescriptor,
    ) -> Vec<ScalarMode> {
        let vertex_count = bowl_mesh.vertex_count();
        if vertex_count < 3 {
            return vec![];
        }

        let mount_anchor = Point3::new(MOUNT_PROFILE_R_M, MOUNT_PROFILE_Y_M, 0.0);
        let constrained = self.mount_constrained_vertices(bowl_mesh, descriptor, mount_anchor);

        let mut lumped_mass = vec![0.0; vertex_count];
        let mut laplacian = DMatrix::zeros(vertex_count, vertex_count);

        for &tri in bowl_mesh.indices_iter() {
            let [ia, ib, ic] = tri;
            let (ia, ib, ic) = (ia as usize, ib as usize, ic as usize);
            if ia >= vertex_count || ib >= vertex_count || ic >= vertex_count {
                continue;
            }

            let pa = bowl_mesh.vertices[ia];
            let pb = bowl_mesh.vertices[ib];
            let pc = bowl_mesh.vertices[ic];
            let Some(area) = triangle_area(&bowl_mesh.vertices, tri) else {
                continue;
            };

            let lump = descriptor.areal_density_kg_per_m2 * area / 3.0;
            lumped_mass[ia] += lump;
            lumped_mass[ib] += lump;
            lumped_mass[ic] += lump;

            let cot_a = cotangent(pa, pb, pc).unwrap_or(0.0).max(0.0);
            let cot_b = cotangent(pb, pc, pa).unwrap_or(0.0).max(0.0);
            let cot_c = cotangent(pc, pa, pb).unwrap_or(0.0).max(0.0);

            accumulate_symmetric_weight(&mut laplacian, ib, ic, 0.5 * cot_a);
            accumulate_symmetric_weight(&mut laplacian, ic, ia, 0.5 * cot_b);
            accumulate_symmetric_weight(&mut laplacian, ia, ib, 0.5 * cot_c);
        }

        let active = (0..vertex_count)
            .filter(|&index| !constrained[index] && lumped_mass[index] > MODAL_EPSILON)
            .collect::<Vec<_>>();
        if active.len() <= MIN_MODE_COUNT {
            return vec![];
        }

        let active_count = active.len();
        let mut reduced_l = DMatrix::zeros(active_count, active_count);
        let mut inv_sqrt_mass = vec![0.0; active_count];

        for (reduced_index, &vertex_index) in active.iter().enumerate() {
            inv_sqrt_mass[reduced_index] = 1.0 / lumped_mass[vertex_index].sqrt();
        }

        for (row_idx, &row_vertex) in active.iter().enumerate() {
            for (col_idx, &col_vertex) in active.iter().enumerate() {
                reduced_l[(row_idx, col_idx)] = laplacian[(row_vertex, col_vertex)];
            }
        }

        let mut scaled_l = reduced_l.clone();
        for row in 0..active_count {
            for col in 0..active_count {
                scaled_l[(row, col)] *= inv_sqrt_mass[row] * inv_sqrt_mass[col];
            }
        }

        let transformed = (&scaled_l * &scaled_l) * descriptor.flexural_rigidity;
        let eigen = transformed.symmetric_eigen();

        let mut solved_modes = Vec::new();
        for eigen_index in 0..eigen.eigenvalues.len() {
            let lambda = eigen.eigenvalues[eigen_index];
            if !lambda.is_finite() || lambda <= MODAL_EPSILON {
                continue;
            }

            let frequency_hz = lambda.sqrt() / (2.0 * std::f64::consts::PI);
            if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
                continue;
            }

            let mut amplitudes = vec![0.0; vertex_count];
            for reduced_index in 0..active_count {
                let vertex_index = active[reduced_index];
                amplitudes[vertex_index] =
                    eigen.eigenvectors[(reduced_index, eigen_index)] * inv_sqrt_mass[reduced_index];
            }

            let max_amp = amplitudes
                .iter()
                .map(|value| value.abs())
                .fold(0.0f64, f64::max)
                .max(MODAL_EPSILON);
            for amplitude in &mut amplitudes {
                *amplitude /= max_amp;
            }

            solved_modes.push(ScalarMode {
                frequency_hz,
                amplitudes,
            });
        }

        solved_modes.sort_by(|left, right| {
            left.frequency_hz
                .partial_cmp(&right.frequency_hz)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        solved_modes.truncate(mode_count);
        solved_modes
    }

    fn mount_constrained_vertices(
        &self,
        bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
        descriptor: BowlDescriptor,
        mount_anchor: Point3<f64>,
    ) -> Vec<bool> {
        let mut constrained = vec![false; bowl_mesh.vertex_count()];
        if bowl_mesh.vertices.is_empty() {
            return constrained;
        }

        let min_distance = bowl_mesh
            .vertices_iter()
            .map(|vertex| (*vertex - mount_anchor).norm())
            .fold(f64::INFINITY, f64::min);
        let support_radius = (descriptor.thickness_m * 2.0)
            .max(descriptor.radius_m * 0.04)
            .max(MODAL_EPSILON);

        let mut constrained_count = 0usize;
        for (index, vertex) in bowl_mesh.vertices_iter().enumerate() {
            let distance = (*vertex - mount_anchor).norm();
            if distance <= min_distance + support_radius {
                constrained[index] = true;
                constrained_count += 1;
            }
        }

        if constrained_count == 0 {
            constrained[nearest_vertex_index(&bowl_mesh.vertices, mount_anchor)] = true;
        }

        constrained
    }

    fn mode_budget(&self, resolution: usize) -> usize {
        (resolution.max(8) / 4).clamp(MIN_MODE_COUNT, MAX_MODE_COUNT)
    }

    fn bowl_descriptor(
        &self,
        resolution: usize,
        bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
    ) -> Option<BowlDescriptor> {
        if bowl_mesh.indices.is_empty() {
            return None;
        }

        let center = self
            .bowl
            .center_of_mass(resolution)
            .unwrap_or_else(|| Point3::new(0.0, 0.0, 0.0));

        let radius_m = {
            let (sum, count) = bowl_mesh
                .vertices
                .iter()
                .fold((0.0, 0usize), |(acc, n), point| {
                    let r = ((point.x - center.x).powi(2) + (point.z - center.z).powi(2)).sqrt();
                    if r.is_finite() {
                        (acc + r, n + 1)
                    } else {
                        (acc, n)
                    }
                });
            (sum / (count.max(1) as f64)).max(MODAL_EPSILON)
        };

        let volume_m3 = self.bowl.meshable.material_volume_m3(resolution);
        let surface_area_m2 =
            mesh_surface_area_m2(&bowl_mesh.vertices, &bowl_mesh.indices).max(MODAL_EPSILON);
        let thickness_m = (volume_m3 / surface_area_m2).max(MODAL_EPSILON);
        let rho = self.bowl.material.density_kg_per_m3.max(MODAL_EPSILON);
        let areal_density_kg_per_m2 = rho * thickness_m;

        let e = self.bowl.material.youngs_modulus_pa.max(1.0e6);
        let nu = self.bowl.material.poisson_ratio.clamp(-0.49, 0.49);
        let flexural_rigidity = e * thickness_m.powi(3) / (12.0 * (1.0 - nu * nu).max(0.05));

        let bowl_mass = self.bowl.mass_kg(resolution).max(MODAL_EPSILON);
        let clapper_mass_ratio = (self.clapper.mass_kg(resolution) / bowl_mass).max(0.0);

        if !(radius_m.is_finite()
            && thickness_m.is_finite()
            && areal_density_kg_per_m2.is_finite()
            && flexural_rigidity.is_finite()
            && clapper_mass_ratio.is_finite())
        {
            return None;
        }

        Some(BowlDescriptor {
            radius_m,
            thickness_m,
            areal_density_kg_per_m2,
            flexural_rigidity,
            clapper_mass_ratio,
        })
    }

    fn mode_damping_for_medium(
        &self,
        mode_index: usize,
        frequency_hz: f64,
        medium: &Medium,
        descriptor: BowlDescriptor,
    ) -> f64 {
        let structural = {
            let stiffness_factor = (1.0e9 / self.bowl.material.youngs_modulus_pa.max(1.0e6)).sqrt();
            let poisson_factor =
                1.0 + self.bowl.material.poisson_ratio.clamp(-0.49, 0.49).abs() * 0.3;
            let mode_factor = 1.0 + mode_index as f64 * 0.05;
            (0.0012 * stiffness_factor * poisson_factor * mode_factor).max(0.0)
        };

        let medium_viscous = medium.viscosity_pa_s.max(0.0)
            / (medium.density_kg_per_m3.max(MODAL_EPSILON)
                * medium.speed_of_sound_m_per_s.max(MODAL_EPSILON)
                * descriptor.thickness_m.max(MODAL_EPSILON));
        let ka = 2.0 * std::f64::consts::PI * frequency_hz * descriptor.radius_m
            / medium.speed_of_sound_m_per_s.max(MODAL_EPSILON);
        let radiation = (ka * ka / (1.0 + ka * ka))
            * (medium.impedance_m_rayl.max(0.0)
                / (medium.impedance_m_rayl.max(0.0) + descriptor.areal_density_kg_per_m2.max(1.0)));

        let coupling = descriptor.clapper_mass_ratio * 0.0025 / ((mode_index + 1) as f64);
        (structural + medium_viscous + radiation * 0.03 + coupling).clamp(1e-5, 0.95)
    }
}

fn average_coupling(
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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModeShape {
    pub vertex_displacement: Vec<Vector3<f64>>,
    pub frequency_hz: f64,
    pub lock_in_range_hz: f64,
    pub angle_sensitivity: f64,
    pub strike_coupling: f64,
    pub jet_coupling: f64,
    pub radiation_efficiency: f64,
    pub damping: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::materials::Material;
    use crate::body::{thickness_map_from_axial_samples, RevolutionAxis, Segment};
    use crate::config::BandChannel;

    fn bowl_material() -> Material {
        Material {
            density_kg_per_m3: 8800.0,
            poisson_ratio: 0.34,
            youngs_modulus_pa: 1.1e11,
        }
    }

    fn clapper_material() -> Material {
        Material {
            density_kg_per_m3: 7850.0,
            poisson_ratio: 0.29,
            youngs_modulus_pa: 2.0e11,
        }
    }

    fn make_bowl() -> Body<BowlGeometry> {
        let seg0 = Segment::start_constant(0.2, 0.14).expect("segment");
        let mut seg1 = Segment::start_constant(0.4, 0.16).expect("segment");
        seg1.start = seg0.end;
        let mut seg2 = Segment::start_constant(0.6, 0.19).expect("segment");
        seg2.start = seg1.end;
        let mut seg3 = Segment::start_constant(0.8, 0.17).expect("segment");
        seg3.start = seg2.end;
        let mut seg4 = Segment::start_constant(1.0, 0.1).expect("segment");
        seg4.start = seg3.end;

        let base = RevolutionMesh::new([seg0, seg1, seg2, seg3, seg4], RevolutionAxis::Y)
            .expect("revolution mesh");
        let axial = base.profile_sample_positions(32);
        let map = thickness_map_from_axial_samples(&axial, 32, |u| {
            (0.006 + 0.002 * u, 0.005 + 0.001 * u)
        })
        .expect("thickness map");
        Body::new(ThickMesh::new(base, map), bowl_material())
    }

    fn make_clapper() -> Body<ClapperGeometry> {
        let seg0 = Segment::start_constant(0.1, 0.03).expect("segment");
        let mut seg1 = Segment::start_constant(0.2, 0.045).expect("segment");
        seg1.start = seg0.end;
        let mut seg2 = Segment::start_constant(0.3, 0.02).expect("segment");
        seg2.start = seg1.end;

        let mesh = RevolutionMesh::new([seg0, seg1, seg2], RevolutionAxis::Y).expect("clapper");
        Body::new(mesh, clapper_material())
    }

    fn make_node() -> Node {
        Node::new(make_bowl(), make_clapper())
    }

    fn band(viscosity_pa_s: f64) -> Band {
        Band {
            channel: BandChannel::Left,
            medium: Medium {
                density_kg_per_m3: 1.225,
                speed_of_sound_m_per_s: 343.0,
                viscosity_pa_s,
                impedance_m_rayl: 420.0,
            },
        }
    }

    #[test]
    fn modal_pipeline_is_deterministic_and_sorted() {
        let node = make_node();
        let resolution = 32;

        let freqs = node.modal_frequencies_hz(resolution);
        assert_eq!(freqs.len(), 8);
        assert!(freqs.iter().all(|f| f.is_finite() && *f > 0.0));
        for pair in freqs.windows(2) {
            assert!(pair[0] <= pair[1]);
        }

        let shapes = node.mode_shapes(resolution);
        assert_eq!(shapes.len(), freqs.len());
        assert!(shapes.iter().all(|m| m.damping >= 0.0));
    }

    #[test]
    fn participation_factors_are_bounded() {
        let node = make_node();
        let resolution = 32;
        let factors = node.modal_participation_factor(
            Point3::new(0.0, 0.6, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            resolution,
        );

        assert_eq!(factors.len(), node.modal_frequencies_hz(resolution).len());
        assert!(factors.iter().all(|f| (0.0..=1.0).contains(f)));
    }

    #[test]
    fn damping_increases_with_viscosity() {
        let node = make_node();
        let resolution = 32;

        let low = node.modal_damping(&band(1.8e-5), resolution);
        let high = node.modal_damping(&band(8.0e-4), resolution);
        assert_eq!(low.len(), high.len());

        let low_sum: f64 = low.iter().sum();
        let high_sum: f64 = high.iter().sum();
        assert!(high_sum > low_sum);
    }

    #[test]
    fn mounting_constraint_reduces_anchor_displacement() {
        let node = make_node();
        let resolution = 32;
        let modes = node.mode_shapes(resolution);
        assert!(!modes.is_empty());

        // Use the same capped resolution that mode_shapes() uses internally, so
        // vertex indices into vertex_displacement are in bounds.
        let mesh = node
            .bowl
            .meshable
            .surface_mesh_data(MODAL_EIGEN_MAX_RESOLUTION)
            .into_parts()
            .0;
        assert!(!mesh.is_empty());

        let anchor = Point3::new(MOUNT_PROFILE_R_M, MOUNT_PROFILE_Y_M, 0.0);
        let anchor_idx = nearest_vertex_index(&mesh, anchor);
        let far_idx = mesh
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                let da = (*a - anchor).norm_squared();
                let db = (*b - anchor).norm_squared();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
            .expect("far vertex");

        let first_mode = &modes[0];
        let anchor_amp = first_mode.vertex_displacement[anchor_idx].norm();
        let far_amp = first_mode.vertex_displacement[far_idx].norm();
        assert!(far_amp >= anchor_amp);
    }
}
