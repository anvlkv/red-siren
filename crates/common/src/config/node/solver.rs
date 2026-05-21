use nalgebra::{DMatrix, Point3};

use crate::{mesh_surface_area_m2, nearest_vertex_index, Meshable, SurfaceMesh};

use super::{
    Node, MIN_MODE_COUNT, MODAL_EIGEN_MAX_RESOLUTION, MODAL_EPSILON, MOUNT_PROFILE_R_M,
    MOUNT_PROFILE_Y_M,
};

/// Shear correction factor for Mindlin-Reissner thick-shell transverse shear (Reissner value).
const MINDLIN_SHEAR_CORRECTION: f64 = 5.0 / 6.0;

/// Cap on the condition number of the kept eigenvalue spectrum.
/// Eigenvalues below `lambda_max / MODAL_CONDITION_NUMBER_CAP` are treated as near-rigid modes
/// and dropped. Replaces the previous ad-hoc `max_lambda * 1e-6` empirical threshold.
const MODAL_CONDITION_NUMBER_CAP: f64 = 1.0e8;

#[derive(Clone, Copy, Debug)]
pub(super) struct BowlDescriptor {
    pub radius_m: f64,
    /// Average shell thickness [m] (volume / surface area). Used as fallback for per-vertex
    /// thickness and retained for backward-compatible acoustics calculations.
    pub thickness_m: f64,
    pub areal_density_kg_per_m2: f64,
    /// Average flexural rigidity D = E·t³/(12(1-ν²)) using average thickness.
    /// Kept for use in acoustics.rs; the solver uses per-vertex D directly.
    pub flexural_rigidity: f64,
    pub clapper_mass_ratio: f64,
    /// Young's modulus [Pa] at the material temperature. Used for per-vertex stiffness.
    pub youngs_modulus_pa: f64,
    /// Poisson ratio (clamped to (-0.49, 0.49)).
    pub poisson_ratio: f64,
    /// Shear modulus G = E/(2(1+ν)) [Pa]. Used for the Mindlin shear stiffness term.
    pub shear_modulus_pa: f64,
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
    /// Condition number of the kept eigenvalue spectrum: lambda_max_kept / lambda_min_kept.
    /// Zero when fewer than two modes are kept.
    pub condition_number: f64,
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
            condition_number: 0.0,
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
        youngs_modulus_pa: e,
        poisson_ratio: nu,
        shear_modulus_pa: e / (2.0 * (1.0 + nu)),
    })
}

/// Compute per-vertex shell thickness [m] from the bowl's `ThicknessMap`.
/// Returns `face_thickness + backface_thickness` for each base-mesh vertex index.
/// If a vertex index is out of range the average descriptor thickness is used as fallback.
pub(super) fn per_vertex_thickness(
    node: &Node,
    vertex_count: usize,
    fallback_thickness: f64,
) -> Vec<f64> {
    (0..vertex_count)
        .map(|i| {
            let (face_t, back_t) = node.bowl.meshable.inner().thickness_map.sample(i);
            (face_t + back_t)
                .max(MODAL_EPSILON)
                .min(fallback_thickness * 10.0)
        })
        .collect()
}

/// Mindlin-Reissner thick-shell modal solver.
///
/// Assembles the generalized eigenproblem `K·u = ω²·M·u` where:
/// - `K = K_b + K_s` — bending stiffness (cotangent Laplacian biharmonic with per-vertex D)
///   plus diagonal lumped shear stiffness (κ·G·t·A_voronoi).
/// - `M` — diagonal lumped mass (per-vertex areal density × Voronoi area), augmented with
///   a uniform clapper-mass perturbation.
///
/// Normalisation: Voronoi areas from the accumulated lumped mass replace the previous
/// ad-hoc global `1/R²` length scale, making the result mesh-resolution-independent.
pub(super) fn solve_scalar_modes(
    mode_count: usize,
    bowl_mesh: &SurfaceMesh<Point3<f64>, [u32; 3]>,
    descriptor: BowlDescriptor,
    pvt: &[f64],
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

    // Material constants for per-vertex stiffness.
    let e = descriptor.youngs_modulus_pa;
    let nu = descriptor.poisson_ratio;
    let denom_bending = 12.0 * (1.0 - nu * nu).max(0.05);
    // Volumetric density from average thickness (used to recover Voronoi area from lumped mass).
    let rho = descriptor.areal_density_kg_per_m2 / descriptor.thickness_m.max(MODAL_EPSILON);

    // Per-vertex flexural rigidity D_i = E·t_i³ / (12(1-ν²)).
    let flex_v: Vec<f64> = (0..vertex_count)
        .map(|i| {
            let t = pvt
                .get(i)
                .copied()
                .unwrap_or(descriptor.thickness_m)
                .max(MODAL_EPSILON);
            (e * t.powi(3) / denom_bending).max(MODAL_EPSILON)
        })
        .collect();

    // Assemble lumped mass (per-vertex t) and cotangent Laplacian (negative cotangents allowed).
    let mut lumped_mass = vec![0.0f64; vertex_count];
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

        // Per-vertex areal density: ρ·t_i (uses per-vertex thickness for mass accuracy).
        let t_a = pvt
            .get(ia)
            .copied()
            .unwrap_or(descriptor.thickness_m)
            .max(MODAL_EPSILON);
        let t_b = pvt
            .get(ib)
            .copied()
            .unwrap_or(descriptor.thickness_m)
            .max(MODAL_EPSILON);
        let t_c = pvt
            .get(ic)
            .copied()
            .unwrap_or(descriptor.thickness_m)
            .max(MODAL_EPSILON);
        lumped_mass[ia] += rho * t_a * area / 3.0;
        lumped_mass[ib] += rho * t_b * area / 3.0;
        lumped_mass[ic] += rho * t_c * area / 3.0;

        // Cotangent weights — negative values allowed (obtuse-triangle correct behaviour).
        let cot_a = cotangent(pa, pb, pc).unwrap_or(0.0);
        let cot_b = cotangent(pb, pc, pa).unwrap_or(0.0);
        let cot_c = cotangent(pc, pa, pb).unwrap_or(0.0);
        accumulate_symmetric_weight(&mut laplacian, ib, ic, 0.5 * cot_a);
        accumulate_symmetric_weight(&mut laplacian, ic, ia, 0.5 * cot_b);
        accumulate_symmetric_weight(&mut laplacian, ia, ib, 0.5 * cot_c);
    }

    // Clapper-mass perturbation: distribute uniformly over all vertices so it is not
    // concentrated on a potentially constrained node. This lowers all frequencies by
    // the factor 1/sqrt(1 + clapper_mass_ratio) independently of mode shape.
    let total_bowl_lumped_mass: f64 = lumped_mass.iter().sum();
    let clapper_mass_kg = descriptor.clapper_mass_ratio * total_bowl_lumped_mass;
    if clapper_mass_kg > MODAL_EPSILON && vertex_count > 0 {
        let per_vertex = clapper_mass_kg / vertex_count as f64;
        for m in &mut lumped_mass {
            *m += per_vertex;
        }
    }

    let active: Vec<usize> = (0..vertex_count)
        .filter(|&i| !constrained[i] && lumped_mass[i] > MODAL_EPSILON)
        .collect();
    let constrained_count = constrained.iter().filter(|c| **c).count();
    if active.len() <= MIN_MODE_COUNT {
        return ScalarSolve {
            modes: vec![],
            constrained_count,
            active_count: active.len(),
            diagnostics: SolverDiagnostics::default(),
        };
    }

    let active_count = active.len();
    let characteristic_edge_length_m = mean_edge_length_m(bowl_mesh).max(MODAL_EPSILON);

    let mut inv_sqrt_mass = vec![0.0f64; active_count];
    for (i_r, &i_v) in active.iter().enumerate() {
        inv_sqrt_mass[i_r] = 1.0 / lumped_mass[i_v].sqrt();
    }

    // Voronoi area at vertex i: A_i ≈ lumped_mass[i] / (ρ·t_i).
    // This is the area element associated with vertex i, used to form the normalised
    // Laplace-Beltrami operator L_norm = L / A and hence K_b = L · diag(D/A) · L.
    let a_voronoi: Vec<f64> = (0..vertex_count)
        .map(|i| {
            let t_i = pvt
                .get(i)
                .copied()
                .unwrap_or(descriptor.thickness_m)
                .max(MODAL_EPSILON);
            let rho_t_i = rho * t_i;
            if rho_t_i > MODAL_EPSILON {
                lumped_mass[i] / rho_t_i
            } else {
                0.0
            }
        })
        .collect();

    // Build the active-submatrix of K_b = L · diag(D_k / A_k) · L.
    // For each (i_r, j_r): K_b[i_r,j_r] = Σ_k L[i_v,k] · (D_k/A_k) · L[k,j_v].
    // The sum over k includes both active and constrained vertices; constrained vertices
    // have w=0 but still contribute through the two-hop Laplacian path. This matches the
    // Galerkin residual with zero Dirichlet BC on constrained nodes.
    let mut reduced_k = DMatrix::zeros(active_count, active_count);
    for (i_r, &i_v) in active.iter().enumerate() {
        for (j_r, &j_v) in active.iter().enumerate() {
            let val: f64 = (0..vertex_count)
                .map(|k| {
                    let a_k = a_voronoi[k];
                    if a_k <= MODAL_EPSILON {
                        return 0.0;
                    }
                    laplacian[(i_v, k)] * (flex_v[k] / a_k) * laplacian[(k, j_v)]
                })
                .sum();
            reduced_k[(i_r, j_r)] = val;
        }
    }

    // Enforce symmetry (floating-point round-off in the triple product).
    for i in 0..active_count {
        for j in (i + 1)..active_count {
            let avg = (reduced_k[(i, j)] + reduced_k[(j, i)]) / 2.0;
            reduced_k[(i, j)] = avg;
            reduced_k[(j, i)] = avg;
        }
    }

    // K_s: diagonal lumped Mindlin-Reissner transverse shear — κ·G·t_i·A_i.
    // Avoids the shear-locking that would arise from a consistent shear interpolation.
    for (i_r, &i_v) in active.iter().enumerate() {
        let t_i = pvt
            .get(i_v)
            .copied()
            .unwrap_or(descriptor.thickness_m)
            .max(MODAL_EPSILON);
        let a_i = a_voronoi[i_v];
        reduced_k[(i_r, i_r)] += MINDLIN_SHEAR_CORRECTION * descriptor.shear_modulus_pa * t_i * a_i;
    }

    // Symmetrised eigenproblem: scaled_k = M^{-1/2} · K · M^{-1/2}.
    // Eigenvalues λ = ω² [rad²/s²]; frequency = √λ / (2π) [Hz].
    let mut scaled_k = reduced_k;
    for row in 0..active_count {
        for col in 0..active_count {
            scaled_k[(row, col)] *= inv_sqrt_mass[row] * inv_sqrt_mass[col];
        }
    }

    let eigen = scaled_k.symmetric_eigen();

    let raw_finite: Vec<f64> = eigen
        .eigenvalues
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
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
        .filter(|v| v.is_finite() && *v > MODAL_EPSILON)
        .fold(0.0f64, f64::max);
    // Condition-number-based threshold: drop modes with ω² < ω²_max / cap.
    let near_rigid_threshold = max_positive_lambda / MODAL_CONDITION_NUMBER_CAP;

    let mut solved_modes: Vec<ScalarMode> = Vec::new();
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

        let mut amplitudes = vec![0.0f64; vertex_count];
        for (r_idx, &v_idx) in active.iter().enumerate() {
            amplitudes[v_idx] = eigen.eigenvectors[(r_idx, eigen_index)] * inv_sqrt_mass[r_idx];
        }

        let max_amp = amplitudes
            .iter()
            .map(|v| v.abs())
            .fold(0.0f64, f64::max)
            .max(MODAL_EPSILON);
        for a in &mut amplitudes {
            *a /= max_amp;
        }

        solved_modes.push(ScalarMode {
            frequency_hz,
            amplitudes,
        });
    }

    solved_modes.sort_by(|l, r| {
        l.frequency_hz
            .partial_cmp(&r.frequency_hz)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    solved_modes.truncate(mode_count);

    let total_lumped_mass_kg = active.iter().copied().map(|i| lumped_mass[i]).sum::<f64>();
    let lambda_min_kept_out = if lambda_min_kept.is_finite() {
        lambda_min_kept
    } else {
        0.0
    };
    let condition_number = if lambda_min_kept_out > MODAL_EPSILON {
        lambda_max_kept / lambda_min_kept_out
    } else {
        0.0
    };

    ScalarSolve {
        modes: solved_modes,
        constrained_count,
        active_count,
        diagnostics: SolverDiagnostics {
            total_lumped_mass_kg,
            characteristic_edge_length_m,
            lambda_min_raw,
            lambda_max_raw,
            lambda_min_kept: lambda_min_kept_out,
            lambda_max_kept,
            dropped_non_finite,
            dropped_non_positive,
            dropped_near_rigid,
            condition_number,
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
    if !weight.is_finite() || weight == 0.0 || i == j {
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
