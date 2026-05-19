use nalgebra::{Point3, Vector3};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct SurfaceMesh<V, I> {
    pub vertices: Vec<V>,
    pub indices: Vec<I>,
}

impl<V, I> SurfaceMesh<V, I> {
    pub fn new(vertices: Vec<V>, indices: Vec<I>) -> Self {
        Self { vertices, indices }
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    pub fn vertices_iter(&self) -> std::slice::Iter<'_, V> {
        self.vertices.iter()
    }

    pub fn indices_iter(&self) -> std::slice::Iter<'_, I> {
        self.indices.iter()
    }

    pub fn into_parts(self) -> (Vec<V>, Vec<I>) {
        (self.vertices, self.indices)
    }
}

impl<V, I> IntoIterator for SurfaceMesh<V, I> {
    type Item = V;
    type IntoIter = std::vec::IntoIter<V>;

    fn into_iter(self) -> Self::IntoIter {
        self.vertices.into_iter()
    }
}

impl<'a, V, I> IntoIterator for &'a SurfaceMesh<V, I> {
    type Item = &'a V;
    type IntoIter = std::slice::Iter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.vertices.iter()
    }
}

/// A generic surface meshing interface for any meshable geometry.
pub trait Meshable {
    type Vertex;
    type Index;
    type Bounds;
    type Vector;

    /// Sample surface vertices given a resolution hint.
    fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex>;

    /// Iterate sampled vertices without requiring downstream call sites to spell out
    /// temporary vector ownership. Default implementation is allocation-backed.
    fn sample_points_iter(&self, resolution: usize) -> std::vec::IntoIter<Self::Vertex> {
        self.sample_points(resolution).into_iter()
    }

    /// Generate triangle index topology consistent with the vertices from `sample_points`.
    fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index>;

    /// Iterate sampled triangle indices. Default implementation is allocation-backed.
    fn mesh_indices_iter(&self, resolution: usize) -> std::vec::IntoIter<Self::Index> {
        self.mesh_indices(resolution).into_iter()
    }

    /// Retrieve vertices and triangle topology together. This allows consumers to sample once
    /// and iterate over both collections repeatedly without re-querying the mesh provider.
    fn surface_mesh_data(&self, resolution: usize) -> SurfaceMesh<Self::Vertex, Self::Index> {
        SurfaceMesh::new(
            self.sample_points(resolution),
            self.mesh_indices(resolution),
        )
    }

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

/// Compute mesh surface area by summing triangle areas.
pub fn mesh_surface_area_m2(points: &[EmbodiedPoint3], indices: &[EmbodiedTriangle]) -> f64 {
    indices
        .iter()
        .filter_map(|[a, b, c]| {
            let (a, b, c) = (*a as usize, *b as usize, *c as usize);
            if a >= points.len() || b >= points.len() || c >= points.len() {
                return None;
            }

            let pa = points[a];
            let pb = points[b];
            let pc = points[c];
            let area = 0.5 * (pb - pa).cross(&(pc - pa)).norm();
            if area.is_finite() {
                Some(area)
            } else {
                None
            }
        })
        .sum()
}

/// Find the closest mesh vertex to the provided point.
pub fn nearest_vertex_index(points: &[EmbodiedPoint3], point: EmbodiedPoint3) -> usize {
    points
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = (*a - point).norm_squared();
            let db = (*b - point).norm_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

/// Area-weighted per-vertex normals from triangle topology.
pub fn mesh_vertex_normals(
    points: &[EmbodiedPoint3],
    indices: &[EmbodiedTriangle],
    epsilon: f64,
) -> Vec<EmbodiedVector3> {
    let mut normals = vec![EmbodiedVector3::zeros(); points.len()];

    for [a, b, c] in indices {
        let (a, b, c) = (*a as usize, *b as usize, *c as usize);
        if a >= points.len() || b >= points.len() || c >= points.len() {
            continue;
        }

        let pa = points[a];
        let pb = points[b];
        let pc = points[c];
        let tri_normal = (pb - pa).cross(&(pc - pa));
        if !tri_normal.iter().all(|v| v.is_finite()) {
            continue;
        }

        normals[a] += tri_normal;
        normals[b] += tri_normal;
        normals[c] += tri_normal;
    }

    for normal in &mut normals {
        *normal = normal
            .try_normalize(epsilon)
            .unwrap_or_else(EmbodiedVector3::zeros);
    }

    normals
}

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

    let mut edges: Vec<(usize, usize)> = edge_counts
        .into_iter()
        .filter_map(|(edge, count)| if count == 1 { Some(edge) } else { None })
        .collect();
    edges.sort_unstable();
    edges
}

fn oriented_boundary_edges(indices: &[EmbodiedTriangle]) -> Vec<(usize, usize)> {
    let mut edge_counts: HashMap<(usize, usize), usize> = HashMap::new();
    let mut orientation: HashMap<(usize, usize), (usize, usize)> = HashMap::new();

    for (a, b) in mesh_edges(indices) {
        let key = if a <= b { (a, b) } else { (b, a) };
        *edge_counts.entry(key).or_insert(0) += 1;
        orientation.entry(key).or_insert((a, b));
    }

    let mut edges: Vec<(usize, usize)> = edge_counts
        .into_iter()
        .filter_map(|(key, count)| {
            if count == 1 {
                orientation.get(&key).copied()
            } else {
                None
            }
        })
        .collect();
    edges.sort_unstable();
    edges
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_surface_area_unit_square_two_triangles() {
        let points = vec![
            EmbodiedPoint3::new(0.0, 0.0, 0.0),
            EmbodiedPoint3::new(1.0, 0.0, 0.0),
            EmbodiedPoint3::new(1.0, 1.0, 0.0),
            EmbodiedPoint3::new(0.0, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2], [0, 2, 3]];

        let area = mesh_surface_area_m2(&points, &indices);
        assert!((area - 1.0).abs() < 1e-12);
    }

    #[test]
    fn nearest_vertex_index_picks_closest_point() {
        let points = vec![
            EmbodiedPoint3::new(0.0, 0.0, 0.0),
            EmbodiedPoint3::new(2.0, 0.0, 0.0),
            EmbodiedPoint3::new(0.0, 2.0, 0.0),
        ];

        let idx = nearest_vertex_index(&points, EmbodiedPoint3::new(1.8, 0.1, 0.0));
        assert_eq!(idx, 1);
    }

    #[test]
    fn mesh_vertex_normals_are_normalized_on_plane() {
        let points = vec![
            EmbodiedPoint3::new(0.0, 0.0, 0.0),
            EmbodiedPoint3::new(1.0, 0.0, 0.0),
            EmbodiedPoint3::new(1.0, 1.0, 0.0),
            EmbodiedPoint3::new(0.0, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2], [0, 2, 3]];

        let normals = mesh_vertex_normals(&points, &indices, 1e-12);
        assert_eq!(normals.len(), points.len());
        for normal in normals {
            assert!((normal.norm() - 1.0).abs() < 1e-12);
            assert!(normal.z.abs() > 0.999999999);
        }
    }

    #[test]
    fn boundary_edges_are_returned_in_sorted_order() {
        let indices = vec![[5, 3, 4]];
        let edges = boundary_edges(&indices);
        assert_eq!(edges, vec![(3, 4), (3, 5), (4, 5)]);
    }

    #[test]
    fn oriented_boundary_edges_are_returned_in_sorted_order() {
        let indices = vec![[5, 3, 4]];
        let edges = oriented_boundary_edges(&indices);
        assert_eq!(edges, vec![(3, 4), (4, 5), (5, 3)]);
    }
}
