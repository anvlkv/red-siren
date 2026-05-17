use nalgebra::{DMatrix, Point3};

use crate::{mesh_surface_area_m2, nearest_vertex_index, Meshable, SurfaceMesh};

use super::{
    Node, MIN_MODE_COUNT, MODAL_EIGEN_MAX_RESOLUTION, MODAL_EPSILON, MOUNT_PROFILE_R_M,
    MOUNT_PROFILE_Y_M,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct BowlDescriptor {
    pub radius_m: f64,
    pub thickness_m: f64,
    pub areal_density_kg_per_m2: f64,
    pub flexural_rigidity: f64,
    pub clapper_mass_ratio: f64,
}

#[derive(Clone, Debug)]
pub(super) struct ScalarMode {
    pub frequency_hz: f64,
    pub amplitudes: Vec<f64>,
}

#[derive(Clone, Debug)]
pub(super) struct ScalarSolve {
    pub modes: Vec<ScalarMode>,
    pub constrained_count: usize,
    pub active_count: usize,
    pub diagnostics: SolverDiagnostics,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SolverDiagnostics {
    pub total_lumped_mass_kg: f64,
    pub characteristic_edge_length_m: f64,
    pub lambda_min_raw: f64,
    pub lambda_max_raw: f64,
    pub lambda_min_kept: f64,
    pub lambda_max_kept: f64,
    pub dropped_non_finite: usize,
    pub dropped_non_positive: usize,
    pub dropped_near_rigid: usize,
}

impl Default for SolverDiagnostics {
    fn default() -> Self {
        Self {
            total_lumped_mass_kg: 0.0,
            characteristic_edge_length_m: 0.0,
            lambda_min_raw: 0.0,
            lambda_max_raw: 0.0,
            lambda_min_kept: 0.0,
            lambda_max_kept: 0.0,
            dropped_non_finite: 0,
            dropped_non_positive: 0,
            dropped_near_rigid: 0,
        }
    }
}

pub(super) fn bowl_descriptor(
    node: &Node,
    resolution: usize,
    bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
    ambient_temperature_c: Option<f64>,
) -> Option<BowlDescriptor> {
    if bowl_mesh.indices.is_empty() {
        return None;
    }

    let center = node
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

    let volume_m3 = node.bowl.meshable.material_volume_m3(resolution);
    let surface_area_m2 =
        mesh_surface_area_m2(&bowl_mesh.vertices, &bowl_mesh.indices).max(MODAL_EPSILON);
    let thickness_m = (volume_m3 / surface_area_m2).max(MODAL_EPSILON);
    let material_temperature_c =
        ambient_temperature_c.unwrap_or(node.bowl.material.reference_temperature_c);
    let rho = node
        .bowl
        .material
        .density_kg_per_m3_at_temperature_c(material_temperature_c)
        .max(MODAL_EPSILON);
    let areal_density_kg_per_m2 = rho * thickness_m;

    let e = node
        .bowl
        .material
        .youngs_modulus_pa_at_temperature_c(material_temperature_c)
        .max(1.0e6);
    let nu = node.bowl.material.poisson_ratio.clamp(-0.49, 0.49);
    let flexural_rigidity = e * thickness_m.powi(3) / (12.0 * (1.0 - nu * nu).max(0.05));

    let bowl_mass = node.bowl.mass_kg(resolution).max(MODAL_EPSILON);
    let clapper_mass_ratio = (node.clapper.mass_kg(resolution) / bowl_mass).max(0.0);

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

pub(super) fn solve_scalar_modes(
    mode_count: usize,
    bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
    descriptor: BowlDescriptor,
) -> ScalarSolve {
    let vertex_count = bowl_mesh.vertex_count();
    if vertex_count < 3 {
        return ScalarSolve {
            modes: vec![],
            constrained_count: 0,
            active_count: 0,
            diagnostics: SolverDiagnostics::default(),
        };
    }

    let mount_anchor = Point3::new(MOUNT_PROFILE_R_M, MOUNT_PROFILE_Y_M, 0.0);
    let constrained = mount_constrained_vertices(bowl_mesh, descriptor, mount_anchor);

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
    let constrained_count = constrained.iter().filter(|is_c| **is_c).count();
    if active.len() <= MIN_MODE_COUNT {
        return ScalarSolve {
            modes: vec![],
            constrained_count,
            active_count: active.len(),
            diagnostics: SolverDiagnostics::default(),
        };
    }

    let active_count = active.len();
    let mut reduced_l = DMatrix::zeros(active_count, active_count);
    let mut inv_sqrt_mass = vec![0.0; active_count];
    let characteristic_edge_length_m = mean_edge_length_m(bowl_mesh).max(MODAL_EPSILON);
    let laplacian_length_scale = 1.0 / characteristic_edge_length_m.powi(2);
    // Thin-shell curvature adds membrane-like restoring behavior that is not captured
    // by a pure plate Laplacian^2 discretization; apply a slenderness-based gain.
    let shell_curvature_gain =
        (descriptor.radius_m / descriptor.thickness_m.max(MODAL_EPSILON)).clamp(1.0, 62.0);

    for (reduced_index, &vertex_index) in active.iter().enumerate() {
        inv_sqrt_mass[reduced_index] = 1.0 / lumped_mass[vertex_index].sqrt();
    }

    for (row_idx, &row_vertex) in active.iter().enumerate() {
        for (col_idx, &col_vertex) in active.iter().enumerate() {
            reduced_l[(row_idx, col_idx)] =
                laplacian[(row_vertex, col_vertex)] * laplacian_length_scale * shell_curvature_gain;
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

    let raw_finite = eigen
        .eigenvalues
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let (lambda_min_raw, lambda_max_raw) = if raw_finite.is_empty() {
        (0.0, 0.0)
    } else {
        (
            raw_finite.iter().copied().fold(f64::INFINITY, f64::min),
            raw_finite.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };

    let max_positive_lambda = eigen
        .eigenvalues
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > MODAL_EPSILON)
        .fold(0.0f64, f64::max);
    let near_rigid_threshold = (max_positive_lambda * 1.0e-6).max(1.0e-3);

    let mut solved_modes = Vec::new();
    let mut lambda_min_kept = f64::INFINITY;
    let mut lambda_max_kept = 0.0f64;
    let mut dropped_non_finite = 0usize;
    let mut dropped_non_positive = 0usize;
    let mut dropped_near_rigid = 0usize;
    for eigen_index in 0..eigen.eigenvalues.len() {
        let lambda = eigen.eigenvalues[eigen_index];
        if !lambda.is_finite() {
            dropped_non_finite += 1;
            continue;
        }
        if lambda <= MODAL_EPSILON {
            dropped_non_positive += 1;
            continue;
        }
        if lambda <= near_rigid_threshold {
            dropped_near_rigid += 1;
            continue;
        }

        let frequency_hz = lambda.sqrt() / (2.0 * std::f64::consts::PI);
        if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
            dropped_non_finite += 1;
            continue;
        }

        lambda_min_kept = lambda_min_kept.min(lambda);
        lambda_max_kept = lambda_max_kept.max(lambda);

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
    let total_lumped_mass_kg = active
        .iter()
        .copied()
        .map(|index| lumped_mass[index])
        .sum::<f64>();
    ScalarSolve {
        modes: solved_modes,
        constrained_count,
        active_count,
        diagnostics: SolverDiagnostics {
            total_lumped_mass_kg,
            characteristic_edge_length_m,
            lambda_min_raw,
            lambda_max_raw,
            lambda_min_kept: if lambda_min_kept.is_finite() {
                lambda_min_kept
            } else {
                0.0
            },
            lambda_max_kept,
            dropped_non_finite,
            dropped_non_positive,
            dropped_near_rigid,
        },
    }
}

pub(super) fn analysis_resolution(node: &Node, resolution: usize) -> usize {
    let requested = resolution.max(8);
    let optimal = node.bowl.meshable.opt_resolution().max(8);
    requested.min(optimal).min(MODAL_EIGEN_MAX_RESOLUTION)
}

fn mount_constrained_vertices(
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

fn mean_edge_length_m(bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>) -> f64 {
    let mut edge_sum = 0.0f64;
    let mut edge_count = 0usize;

    for &[ia, ib, ic] in bowl_mesh.indices_iter() {
        let tri = [ia as usize, ib as usize, ic as usize];
        if tri.iter().any(|index| *index >= bowl_mesh.vertices.len()) {
            continue;
        }

        let pa = bowl_mesh.vertices[tri[0]];
        let pb = bowl_mesh.vertices[tri[1]];
        let pc = bowl_mesh.vertices[tri[2]];
        let edges = [(pa - pb).norm(), (pb - pc).norm(), (pc - pa).norm()];
        for edge in edges {
            if edge.is_finite() && edge > MODAL_EPSILON {
                edge_sum += edge;
                edge_count += 1;
            }
        }
    }

    if edge_count == 0 {
        0.0
    } else {
        edge_sum / edge_count as f64
    }
}
