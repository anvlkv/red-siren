use crate::body::meshable::{
    boundary_edges, cavity_volume_from_shell, mesh_signed_volume, EmbodiedBounds, EmbodiedPoint3,
    EmbodiedTriangle, EmbodiedVector3, Meshable,
};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const DIRECTION_EPSILON: f64 = 1e-9;
const EDGE_TAPER_RINGS: usize = 2;
const RAY_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThicknessMapPoint {
    pub body_vertex_index: usize,
    pub face_thickness: f64,
    pub backface_thickness: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThicknessMap {
    pub points: Vec<ThicknessMapPoint>,
}

/// Build a `ThicknessMap` by evaluating a thickness function over axial positions.
///
/// `axial_positions` is expected to come from a profile sampler such as
/// `RevolutionBody::profile_sample_positions`. The generated map repeats each
/// axial thickness value across all theta samples to match revolution mesh
/// indexing (`j * axial_len + i`).
pub fn thickness_map_from_axial_samples<F>(
    axial_positions: &[f64],
    theta_samples: usize,
    mut thickness_at_u: F,
) -> Option<ThicknessMap>
where
    F: FnMut(f64) -> (f64, f64),
{
    if axial_positions.is_empty() || theta_samples == 0 {
        return None;
    }

    let axial_len = axial_positions.len();
    let mut points = Vec::with_capacity(axial_len * theta_samples);

    for j in 0..theta_samples {
        for (i, u) in axial_positions.iter().enumerate() {
            let (face_thickness, backface_thickness) = thickness_at_u(u.clamp(0.0, 1.0));
            points.push(ThicknessMapPoint {
                body_vertex_index: j * axial_len + i,
                face_thickness,
                backface_thickness,
            });
        }
    }

    ThicknessMap::new(points)
}

impl ThicknessMap {
    pub fn new(points: Vec<ThicknessMapPoint>) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        if !points
            .iter()
            .all(|point| point.face_thickness.is_finite() && point.backface_thickness.is_finite())
        {
            return None;
        }

        Some(Self {
            points: normalize_points(points),
        })
    }

    pub fn sample(&self, body_vertex_index: usize) -> (f64, f64) {
        let first = &self.points[0];
        if body_vertex_index <= first.body_vertex_index {
            return (first.face_thickness, first.backface_thickness);
        }

        let last = &self.points[self.points.len() - 1];
        if body_vertex_index >= last.body_vertex_index {
            return (last.face_thickness, last.backface_thickness);
        }

        for window in self.points.windows(2) {
            let left = &window[0];
            let right = &window[1];
            if body_vertex_index >= left.body_vertex_index
                && body_vertex_index <= right.body_vertex_index
            {
                let span = right.body_vertex_index - left.body_vertex_index;
                if span == 0 {
                    return (right.face_thickness, right.backface_thickness);
                }

                let t = (body_vertex_index - left.body_vertex_index) as f64 / span as f64;
                let face = left.face_thickness + (right.face_thickness - left.face_thickness) * t;
                let backface = left.backface_thickness
                    + (right.backface_thickness - left.backface_thickness) * t;
                return (face, backface);
            }
        }

        (last.face_thickness, last.backface_thickness)
    }
}

fn normalize_points(mut points: Vec<ThicknessMapPoint>) -> Vec<ThicknessMapPoint> {
    points.sort_by_key(|point| point.body_vertex_index);

    let mut normalized: Vec<ThicknessMapPoint> = Vec::with_capacity(points.len());
    for point in points {
        if let Some(last) = normalized.last_mut() {
            if last.body_vertex_index == point.body_vertex_index {
                last.face_thickness = point.face_thickness;
                last.backface_thickness = point.backface_thickness;
                continue;
            }
        }

        normalized.push(ThicknessMapPoint {
            body_vertex_index: point.body_vertex_index,
            face_thickness: point.face_thickness,
            backface_thickness: point.backface_thickness,
        });
    }

    normalized
}

fn boundary_taper_factors(
    indices: &[EmbodiedTriangle],
    vertex_count: usize,
    taper_rings: usize,
) -> Vec<f64> {
    if vertex_count == 0 {
        return vec![];
    }

    if taper_rings == 0 {
        return vec![1.0; vertex_count];
    }

    let boundary = boundary_edges(indices);
    if boundary.is_empty() {
        return vec![1.0; vertex_count];
    }

    let mut adjacency = vec![Vec::<usize>::new(); vertex_count];
    for [a, b, c] in indices {
        let tri_edges = [
            (*a as usize, *b as usize),
            (*b as usize, *c as usize),
            (*c as usize, *a as usize),
        ];
        for (start, end) in tri_edges {
            if start < vertex_count && end < vertex_count {
                adjacency[start].push(end);
                adjacency[end].push(start);
            }
        }
    }

    let mut distance = vec![usize::MAX; vertex_count];
    let mut queue = VecDeque::new();
    for (a, b) in boundary {
        if a < vertex_count && distance[a] == usize::MAX {
            distance[a] = 0;
            queue.push_back(a);
        }
        if b < vertex_count && distance[b] == usize::MAX {
            distance[b] = 0;
            queue.push_back(b);
        }
    }

    while let Some(v) = queue.pop_front() {
        let next_dist = distance[v].saturating_add(1);
        if next_dist > taper_rings {
            continue;
        }

        for &neighbor in &adjacency[v] {
            if distance[neighbor] > next_dist {
                distance[neighbor] = next_dist;
                queue.push_back(neighbor);
            }
        }
    }

    distance
        .into_iter()
        .map(|d| {
            if d == usize::MAX || d >= taper_rings {
                1.0
            } else {
                d as f64 / taper_rings as f64
            }
        })
        .collect()
}

fn incident_triangles(
    indices: &[EmbodiedTriangle],
    vertex_index: usize,
) -> Vec<(usize, [usize; 3])> {
    indices
        .iter()
        .enumerate()
        .filter_map(|(tri_idx, [a, b, c])| {
            let tri = [*a as usize, *b as usize, *c as usize];
            if tri.contains(&vertex_index) {
                Some((tri_idx, tri))
            } else {
                None
            }
        })
        .collect()
}

fn area_weighted_vertex_normal(
    points: &[EmbodiedPoint3],
    indices: &[EmbodiedTriangle],
    vertex_index: usize,
) -> Option<Vector3<f64>> {
    if vertex_index >= points.len() {
        return None;
    }

    let mut normal = Vector3::zeros();
    for (_tri_idx, [a, b, c]) in incident_triangles(indices, vertex_index) {
        if a >= points.len() || b >= points.len() || c >= points.len() {
            continue;
        }
        let pa = points[a];
        let pb = points[b];
        let pc = points[c];
        let tri_normal = (pb - pa).cross(&(pc - pa));
        if tri_normal.norm_squared().is_finite() {
            normal += tri_normal;
        }
    }

    normal.try_normalize(RAY_EPSILON)
}

fn ray_triangle_hit_distance(
    origin: EmbodiedPoint3,
    direction: Vector3<f64>,
    p0: EmbodiedPoint3,
    p1: EmbodiedPoint3,
    p2: EmbodiedPoint3,
) -> Option<f64> {
    let e1 = p1 - p0;
    let e2 = p2 - p0;
    let h = direction.cross(&e2);
    let a = e1.dot(&h);
    if a.abs() < RAY_EPSILON {
        return None;
    }

    let f = 1.0 / a;
    let s = origin - p0;
    let u = f * s.dot(&h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(&e1);
    let v = f * direction.dot(&q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * e2.dot(&q);
    if t > RAY_EPSILON {
        Some(t)
    } else {
        None
    }
}

fn directional_thickness_from_mesh(
    points: &[EmbodiedPoint3],
    indices: &[EmbodiedTriangle],
    vertex_index: usize,
    direction: Vector3<f64>,
) -> Option<f64> {
    if vertex_index >= points.len() {
        return None;
    }
    let dir = direction.try_normalize(RAY_EPSILON)?;
    let origin = points[vertex_index];

    let incident: std::collections::HashSet<usize> = incident_triangles(indices, vertex_index)
        .into_iter()
        .map(|(idx, _)| idx)
        .collect();

    indices
        .iter()
        .enumerate()
        .filter(|(tri_idx, _)| !incident.contains(tri_idx))
        .filter_map(|(_tri_idx, [a, b, c])| {
            let (a, b, c) = (*a as usize, *b as usize, *c as usize);
            if a >= points.len() || b >= points.len() || c >= points.len() {
                return None;
            }
            ray_triangle_hit_distance(origin, dir, points[a], points[b], points[c])
        })
        .min_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThickMesh<B> {
    pub base: B,
    pub thickness_map: ThicknessMap,
}

impl<B> ThickMesh<B> {
    pub fn new(base: B, thickness_map: ThicknessMap) -> Self {
        Self {
            base,
            thickness_map,
        }
    }
}

impl<B> Meshable for ThickMesh<B>
where
    B: Meshable<
        Vertex = EmbodiedPoint3,
        Index = EmbodiedTriangle,
        Bounds = EmbodiedBounds,
        Vector = EmbodiedVector3,
    >,
{
    type Vertex = EmbodiedPoint3;
    type Index = EmbodiedTriangle;
    type Bounds = EmbodiedBounds;
    type Vector = EmbodiedVector3;

    fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex> {
        let base_points = self.base.sample_points(resolution);
        if base_points.is_empty() {
            return vec![];
        }
        let base_indices = self.base.mesh_indices(resolution);
        let taper = boundary_taper_factors(&base_indices, base_points.len(), EDGE_TAPER_RINGS);

        let (min, max) = self.base.bounding_box(resolution.max(2));
        let center = EmbodiedPoint3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (min.z + max.z) * 0.5,
        );

        let mut points = Vec::with_capacity(base_points.len() * 2);

        for (index, base_point) in base_points.iter().enumerate() {
            let (face_thickness, backface_thickness) = self.thickness_map.sample(index);
            let taper_factor = taper.get(index).copied().unwrap_or(1.0);
            let face_thickness = face_thickness * taper_factor;
            let backface_thickness = backface_thickness * taper_factor;

            let direction_vec = base_point - center;
            let direction = direction_vec
                .try_normalize(DIRECTION_EPSILON)
                .unwrap_or_else(|| nalgebra::Vector3::new(0.0, 1.0, 0.0));

            points.push(*base_point + direction * face_thickness);
            points.push(*base_point - direction * backface_thickness);
        }

        points
    }

    fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index> {
        let base_indices = self.base.mesh_indices(resolution);
        let base_vertex_count = self.base.sample_points(resolution).len() as u32;
        if base_indices.is_empty() || base_vertex_count == 0 {
            return vec![];
        }
        let bridges = boundary_edges(&base_indices);

        // Triangle order is stable: outer shell, inner shell, then boundary bridges.
        let mut indices = Vec::with_capacity(base_indices.len() * 2 + bridges.len() * 2);
        for [a, b, c] in &base_indices {
            indices.push([a * 2, b * 2, c * 2]);
            indices.push([c * 2 + 1, b * 2 + 1, a * 2 + 1]);
        }

        for (a, b) in bridges {
            let a = a as u32;
            let b = b as u32;
            indices.push([a * 2, b * 2, b * 2 + 1]);
            indices.push([a * 2, b * 2 + 1, a * 2 + 1]);
        }

        indices
    }

    fn bounding_box(&self, resolution: usize) -> Self::Bounds {
        let points = self.sample_points(resolution.max(2));
        if points.is_empty() {
            let zero = EmbodiedPoint3::origin();
            return (zero, zero);
        }

        let mut min = points[0];
        let mut max = points[0];

        for point in &points {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            min.z = min.z.min(point.z);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
            max.z = max.z.max(point.z);
        }

        (min, max)
    }

    fn material_volume_m3(&self, resolution: usize) -> f64 {
        let points = self.sample_points(resolution.max(2));
        if points.is_empty() {
            return 0.0;
        }

        let indices = self.mesh_indices(resolution.max(2));
        if indices.is_empty() {
            return 0.0;
        }

        mesh_signed_volume(&points, &indices).abs()
    }

    fn cavity_volume_m3(&self, resolution: usize) -> Option<f64> {
        let resolution = resolution.max(2);
        let base_indices = self.base.mesh_indices(resolution);
        if base_indices.is_empty() {
            return None;
        }

        let points = self.sample_points(resolution);
        if points.is_empty() {
            return None;
        }

        let thick_indices = self.mesh_indices(resolution);
        let inner_start = base_indices.len();
        let inner_end = inner_start + base_indices.len();
        if thick_indices.len() < inner_end {
            return None;
        }

        cavity_volume_from_shell(&points, &thick_indices[inner_start..inner_end])
    }

    fn surface_normal_at_vertex(
        &self,
        resolution: usize,
        vertex_index: usize,
    ) -> Option<Vector3<f64>> {
        let points = self.sample_points(resolution.max(3));
        if points.is_empty() {
            return None;
        }
        let indices = self.mesh_indices(resolution.max(3));
        area_weighted_vertex_normal(&points, &indices, vertex_index)
    }

    fn material_thickness_at_vertex(
        &self,
        resolution: usize,
        vertex_index: usize,
        direction: Vector3<f64>,
    ) -> Option<f64> {
        let points = self.sample_points(resolution.max(3));
        if points.is_empty() {
            return None;
        }
        let indices = self.mesh_indices(resolution.max(3));
        directional_thickness_from_mesh(&points, &indices, vertex_index, direction)
    }

    fn opt_resolution(&self) -> usize {
        self.base.opt_resolution()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{RevolutionAxis, RevolutionMesh, Segment};

    #[derive(Clone, Debug)]
    struct ClosedTetra;

    impl Meshable for ClosedTetra {
        type Vertex = EmbodiedPoint3;
        type Index = EmbodiedTriangle;
        type Bounds = EmbodiedBounds;
        type Vector = EmbodiedVector3;

        fn sample_points(&self, _resolution: usize) -> Vec<Self::Vertex> {
            vec![
                EmbodiedPoint3::new(0.0, 0.0, 0.0),
                EmbodiedPoint3::new(1.0, 0.0, 0.0),
                EmbodiedPoint3::new(0.0, 1.0, 0.0),
                EmbodiedPoint3::new(0.0, 0.0, 1.0),
            ]
        }

        fn mesh_indices(&self, _resolution: usize) -> Vec<Self::Index> {
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]
        }

        fn bounding_box(&self, _resolution: usize) -> Self::Bounds {
            (
                EmbodiedPoint3::new(0.0, 0.0, 0.0),
                EmbodiedPoint3::new(1.0, 1.0, 1.0),
            )
        }

        fn material_volume_m3(&self, _resolution: usize) -> f64 {
            1.0 / 6.0
        }

        fn cavity_volume_m3(&self, _resolution: usize) -> Option<f64> {
            None
        }

        fn surface_normal_at_vertex(
            &self,
            _resolution: usize,
            vertex_index: usize,
        ) -> Option<Self::Vector> {
            let points = self.sample_points(4);
            let indices = self.mesh_indices(4);
            area_weighted_vertex_normal(&points, &indices, vertex_index)
        }

        fn material_thickness_at_vertex(
            &self,
            _resolution: usize,
            _vertex_index: usize,
            _direction: Self::Vector,
        ) -> Option<f64> {
            None
        }

        fn opt_resolution(&self) -> usize {
            4
        }
    }

    fn make_body() -> RevolutionMesh<1> {
        let segment = Segment::start_constant(1.0, 1.0).expect("segment");
        RevolutionMesh::new([segment], RevolutionAxis::Y).expect("body")
    }

    #[test]
    fn thickness_map_interpolates_and_holds_endpoints() {
        let map = ThicknessMap::new(vec![
            ThicknessMapPoint {
                body_vertex_index: 2,
                face_thickness: 0.1,
                backface_thickness: 0.2,
            },
            ThicknessMapPoint {
                body_vertex_index: 6,
                face_thickness: 0.5,
                backface_thickness: 0.4,
            },
        ])
        .expect("map");

        assert_eq!(map.sample(0), (0.1, 0.2));
        assert_eq!(map.sample(10), (0.5, 0.4));

        let mid = map.sample(4);
        assert!((mid.0 - 0.3).abs() < 1e-10);
        assert!((mid.1 - 0.3).abs() < 1e-10);
    }

    #[test]
    fn thickness_map_normalizes_unsorted_duplicate_index() {
        let map = ThicknessMap::new(vec![
            ThicknessMapPoint {
                body_vertex_index: 4,
                face_thickness: 0.4,
                backface_thickness: 0.5,
            },
            ThicknessMapPoint {
                body_vertex_index: 2,
                face_thickness: 0.1,
                backface_thickness: 0.2,
            },
            ThicknessMapPoint {
                body_vertex_index: 2,
                face_thickness: 0.3,
                backface_thickness: 0.6,
            },
        ])
        .expect("map");

        assert_eq!(map.points.len(), 2);
        assert_eq!(map.points[0].body_vertex_index, 2);
        assert_eq!(map.points[0].face_thickness, 0.3);
        assert_eq!(map.points[0].backface_thickness, 0.6);
    }

    #[test]
    fn thickness_map_from_axial_samples_expands_across_theta() {
        let map = thickness_map_from_axial_samples(&[0.0, 0.5, 1.0], 2, |u| (u * 0.4, u * 0.2))
            .expect("map");

        assert_eq!(map.points.len(), 6);
        assert_eq!(map.points[0].body_vertex_index, 0);
        assert_eq!(map.points[2].body_vertex_index, 2);
        assert_eq!(map.points[3].body_vertex_index, 3);

        let (face_0, back_0) = map.sample(0);
        assert!((face_0 - 0.0).abs() < 1e-10);
        assert!((back_0 - 0.0).abs() < 1e-10);

        let (face_mid, back_mid) = map.sample(4);
        assert!((face_mid - 0.2).abs() < 1e-10);
        assert!((back_mid - 0.1).abs() < 1e-10);

        let (face_end, back_end) = map.sample(5);
        assert!((face_end - 0.4).abs() < 1e-10);
        assert!((back_end - 0.2).abs() < 1e-10);
    }

    #[test]
    fn thickness_map_from_axial_samples_rejects_invalid_input() {
        assert!(thickness_map_from_axial_samples(&[], 8, |_u| (0.1, 0.1)).is_none());
        assert!(thickness_map_from_axial_samples(&[0.0, 1.0], 0, |_u| (0.1, 0.1)).is_none());
        assert!(thickness_map_from_axial_samples(&[0.0, 1.0], 2, |_u| (f64::NAN, 0.1)).is_none());
    }

    #[test]
    fn thick_body_emits_two_layers_and_indices() {
        let base = make_body();
        let resolution = 8;
        let base_points = base.sample_points(resolution);
        let base_indices = base.mesh_indices(resolution);
        let base_boundary_edges = boundary_edges(&base_indices);

        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.05,
            backface_thickness: 0.03,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        let points = thick.sample_points(resolution);
        let indices = thick.mesh_indices(resolution);

        assert_eq!(points.len(), base_points.len() * 2);
        assert_eq!(
            indices.len(),
            base_indices.len() * 2 + base_boundary_edges.len() * 2
        );

        let vertex_count = points.len() as u32;
        for [a, b, c] in indices {
            assert!(a < vertex_count);
            assert!(b < vertex_count);
            assert!(c < vertex_count);
        }
    }

    #[test]
    fn thick_body_tapers_to_base_on_open_edges() {
        let base = make_body();
        let resolution = 8;
        let base_points = base.sample_points(resolution);
        let base_indices = base.mesh_indices(resolution);
        let boundary = boundary_edges(&base_indices);

        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.2,
            backface_thickness: 0.2,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);
        let thick_points = thick.sample_points(resolution);

        let mut boundary_vertex_seen = vec![false; base_points.len()];
        for (a, b) in boundary {
            boundary_vertex_seen[a] = true;
            boundary_vertex_seen[b] = true;
        }

        for (i, base_point) in base_points.iter().enumerate() {
            if !boundary_vertex_seen[i] {
                continue;
            }

            let face = thick_points[i * 2];
            let back = thick_points[i * 2 + 1];
            assert!((face - *base_point).norm() < 1e-10);
            assert!((back - *base_point).norm() < 1e-10);
        }
    }

    #[test]
    fn thick_body_material_volume_is_positive() {
        let base = make_body();
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.08,
            backface_thickness: 0.04,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        assert!(thick.material_volume_m3(16) > 0.0);
    }

    #[test]
    fn thick_body_cavity_is_positive_for_open_base() {
        let base = make_body();
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.08,
            backface_thickness: 0.04,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        let cavity = thick.cavity_volume_m3(64).expect("cavity");
        assert!(cavity.is_finite());
        assert!(cavity > 0.0);
        assert!(cavity < std::f64::consts::PI);
    }

    #[test]
    fn thick_body_cavity_is_positive_for_bowl_profile() {
        let segment = Segment::start_parabolic(1.0, 0.35, 0.0, 0.55).expect("segment");
        let base = RevolutionMesh::new([segment], RevolutionAxis::Y).expect("body");
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.06,
            backface_thickness: 0.05,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        let cavity = thick.cavity_volume_m3(96).expect("cavity");
        assert!(cavity.is_finite());
        assert!(cavity > 0.0);
    }

    #[test]
    fn thick_body_cavity_is_some_for_closed_base() {
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.05,
            backface_thickness: 0.03,
        }])
        .expect("map");
        let thick = ThickMesh::new(ClosedTetra, map);

        let cavity = thick.cavity_volume_m3(8).expect("cavity");
        assert!(cavity > 0.0);
    }

    #[test]
    fn thick_body_surface_normal_invalid_vertex_is_none() {
        let base = make_body();
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.05,
            backface_thickness: 0.05,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        assert_eq!(thick.surface_normal_at_vertex(16, usize::MAX), None);
    }

    #[test]
    fn thick_body_thickness_rejects_zero_direction() {
        let base = make_body();
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.08,
            backface_thickness: 0.04,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        assert_eq!(
            thick.material_thickness_at_vertex(16, 0, Vector3::zeros()),
            None
        );
    }

    #[test]
    fn thick_body_thickness_positive_for_inward_probe() {
        let base = make_body();
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.08,
            backface_thickness: 0.04,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        let points = thick.sample_points(16);
        let p0 = points[0];
        let inward = Vector3::new(-p0.x, -p0.y, -p0.z);
        let t = thick
            .material_thickness_at_vertex(16, 0, inward)
            .expect("thickness");
        assert!(t > 0.0);
    }

    #[test]
    fn thick_mesh_indices_are_deterministic_across_calls() {
        let base = make_body();
        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.05,
            backface_thickness: 0.04,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);

        let first = thick.mesh_indices(16);
        let second = thick.mesh_indices(16);
        assert_eq!(first, second);
    }

    #[test]
    fn thick_mesh_bridge_indices_use_vertex_pair_layout() {
        let base = make_body();
        let resolution = 8;
        let base_indices = base.mesh_indices(resolution);
        let bridges = boundary_edges(&base_indices);
        let base_vertex_count = base.sample_points(resolution).len() as u32;

        let map = ThicknessMap::new(vec![ThicknessMapPoint {
            body_vertex_index: 0,
            face_thickness: 0.05,
            backface_thickness: 0.05,
        }])
        .expect("map");
        let thick = ThickMesh::new(base, map);
        let indices = thick.mesh_indices(resolution);

        let bridge_start = base_indices.len() * 2;
        let bridge_indices = &indices[bridge_start..];
        assert_eq!(bridge_indices.len(), bridges.len() * 2);

        for (i, (a, b)) in bridges.iter().enumerate() {
            let a = *a as u32;
            let b = *b as u32;
            let tri0 = bridge_indices[i * 2];
            let tri1 = bridge_indices[i * 2 + 1];

            assert_eq!(tri0, [a * 2, b * 2, b * 2 + 1]);
            assert_eq!(tri1, [a * 2, b * 2 + 1, a * 2 + 1]);

            for v in tri0.into_iter().chain(tri1) {
                assert!(v < base_vertex_count * 2);
                assert!((v / 2) < base_vertex_count);
            }
        }
    }
}
