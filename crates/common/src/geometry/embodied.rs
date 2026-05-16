use nalgebra::{Point3, Vector3};
use std::collections::HashMap;

/// A generic surface meshing interface for any meshable geometry.
pub trait Embodied {
    type Vertex;
    type Index;
    type Bounds;
    type Vector;

    /// Sample surface vertices given a resolution hint.
    fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex>;

    /// Generate triangle index topology consistent with the vertices from `sample_points`.
    fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index>;

    /// Compute the axis-aligned bounding box of the surface.
    fn bounding_box(&self, resolution: usize) -> Self::Bounds;

    /// Compute material volume in cubic meters (m^3) using the given resolution.
    fn material_volume_m3(&self, resolution: usize) -> f64;

    /// Compute enclosed cavity volume in cubic meters (m^3), if well-defined.
    fn cavity_volume_m3(&self, resolution: usize) -> Option<f64>;

    /// Compute a surface normal at a sampled vertex index.
    fn surface_normal_at_vertex(
        &self,
        resolution: usize,
        vertex_index: usize,
    ) -> Option<Self::Vector>;

    /// Compute material thickness at a sampled vertex index in the given direction.
    fn material_thickness_at_vertex(
        &self,
        resolution: usize,
        vertex_index: usize,
        direction: Self::Vector,
    ) -> Option<f64>;

    /// Optimal resolution hint for sampling this geometry, if any. This can be used by downstream code to avoid unnecessary sampling at very high resolutions.
    fn opt_resolution(&self) -> usize;
}

/// Default mesh output aliases used by RevolutionBody.
pub type EmbodiedPoint3 = Point3<f64>;
pub type EmbodiedTriangle = [u32; 3];
pub type EmbodiedBounds = (Point3<f64>, Point3<f64>);
pub type EmbodiedVector3 = Vector3<f64>;

pub(crate) fn mesh_signed_volume(points: &[EmbodiedPoint3], indices: &[EmbodiedTriangle]) -> f64 {
    indices
        .iter()
        .filter_map(|[a, b, c]| {
            let (a, b, c) = (*a as usize, *b as usize, *c as usize);
            if a >= points.len() || b >= points.len() || c >= points.len() {
                return None;
            }

            let p0 = points[a];
            let p1 = points[b];
            let p2 = points[c];
            if !(p0.x.is_finite()
                && p0.y.is_finite()
                && p0.z.is_finite()
                && p1.x.is_finite()
                && p1.y.is_finite()
                && p1.z.is_finite()
                && p2.x.is_finite()
                && p2.y.is_finite()
                && p2.z.is_finite())
            {
                return None;
            }

            Some(p0.coords.dot(&p1.coords.cross(&p2.coords)) / 6.0)
        })
        .sum()
}

fn mesh_edges(indices: &[EmbodiedTriangle]) -> Vec<(usize, usize)> {
    let mut edges = Vec::with_capacity(indices.len() * 3);
    for [a, b, c] in indices {
        edges.push((*a as usize, *b as usize));
        edges.push((*b as usize, *c as usize));
        edges.push((*c as usize, *a as usize));
    }
    edges
}

pub(crate) fn boundary_edges(indices: &[EmbodiedTriangle]) -> Vec<(usize, usize)> {
    let mut edge_counts: HashMap<(usize, usize), usize> = HashMap::new();
    for (a, b) in mesh_edges(indices) {
        let key = if a <= b { (a, b) } else { (b, a) };
        *edge_counts.entry(key).or_insert(0) += 1;
    }

    edge_counts
        .into_iter()
        .filter_map(|(edge, count)| if count == 1 { Some(edge) } else { None })
        .collect()
}

fn oriented_boundary_edges(indices: &[EmbodiedTriangle]) -> Vec<(usize, usize)> {
    let mut edge_counts: HashMap<(usize, usize), usize> = HashMap::new();
    let mut orientation: HashMap<(usize, usize), (usize, usize)> = HashMap::new();

    for (a, b) in mesh_edges(indices) {
        let key = if a <= b { (a, b) } else { (b, a) };
        *edge_counts.entry(key).or_insert(0) += 1;
        orientation.entry(key).or_insert((a, b));
    }

    edge_counts
        .into_iter()
        .filter_map(|(key, count)| {
            if count == 1 {
                orientation.get(&key).copied()
            } else {
                None
            }
        })
        .collect()
}

fn ordered_boundary_loops(indices: &[EmbodiedTriangle]) -> Option<Vec<Vec<usize>>> {
    let oriented = oriented_boundary_edges(indices);
    if oriented.is_empty() {
        return Some(vec![]);
    }

    let mut outgoing: HashMap<usize, Vec<usize>> = HashMap::new();
    for (start, end) in &oriented {
        outgoing.entry(*start).or_default().push(*end);
    }

    let mut used: HashMap<(usize, usize), bool> =
        oriented.iter().copied().map(|e| (e, false)).collect();
    let mut loops = Vec::new();

    for &(start, next) in &oriented {
        if used.get(&(start, next)).copied().unwrap_or(false) {
            continue;
        }

        let mut loop_vertices = vec![start];
        let mut current = start;
        let mut candidate = next;

        loop {
            if let Some(entry) = used.get_mut(&(current, candidate)) {
                *entry = true;
            } else {
                return None;
            }

            if candidate == start {
                break;
            }

            loop_vertices.push(candidate);
            let next_options = outgoing.get(&candidate)?;
            let next_edge = next_options
                .iter()
                .copied()
                .find(|&edge_end| !used.get(&(candidate, edge_end)).copied().unwrap_or(false))?;

            current = candidate;
            candidate = next_edge;
        }

        if loop_vertices.len() < 3 {
            return None;
        }

        loops.push(loop_vertices);
    }

    if used.values().any(|seen| !seen) {
        return None;
    }

    Some(loops)
}

pub(crate) fn cavity_volume_from_shell(
    points: &[EmbodiedPoint3],
    indices: &[EmbodiedTriangle],
) -> Option<f64> {
    if points.is_empty() || indices.is_empty() {
        return None;
    }

    let loops = ordered_boundary_loops(indices)?;
    let mut closed_points = points.to_vec();
    let mut closed_indices = indices.to_vec();

    for loop_vertices in loops {
        let mut centroid = Vector3::zeros();
        for &vertex_index in &loop_vertices {
            let point = *closed_points.get(vertex_index)?;
            centroid += point.coords;
        }
        centroid /= loop_vertices.len() as f64;

        if !centroid.iter().all(|component| component.is_finite()) {
            return None;
        }

        let center_index = closed_points.len() as u32;
        closed_points.push(Point3::from(centroid));

        let mut forward_cap = Vec::with_capacity(loop_vertices.len());
        let mut reverse_cap = Vec::with_capacity(loop_vertices.len());

        for i in 0..loop_vertices.len() {
            let a = loop_vertices[i] as u32;
            let b = loop_vertices[(i + 1) % loop_vertices.len()] as u32;
            forward_cap.push([center_index, a, b]);
            reverse_cap.push([center_index, b, a]);
        }

        let mut forward_indices = closed_indices.clone();
        forward_indices.extend_from_slice(&forward_cap);
        let mut reverse_indices = closed_indices.clone();
        reverse_indices.extend_from_slice(&reverse_cap);

        if mesh_signed_volume(&closed_points, &forward_indices).abs()
            >= mesh_signed_volume(&closed_points, &reverse_indices).abs()
        {
            closed_indices = forward_indices;
        } else {
            closed_indices = reverse_indices;
        }
    }

    let volume = mesh_signed_volume(&closed_points, &closed_indices).abs();
    if volume.is_finite() {
        Some(volume)
    } else {
        None
    }
}
