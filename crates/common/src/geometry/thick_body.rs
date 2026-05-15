use crate::geometry::embodied::{Embodied, EmbodiedBounds, EmbodiedPoint3, EmbodiedTriangle};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

const DIRECTION_EPSILON: f64 = 1e-9;
const EDGE_TAPER_RINGS: usize = 2;

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

fn mesh_edges(indices: &[EmbodiedTriangle]) -> Vec<(usize, usize)> {
    let mut edges = Vec::with_capacity(indices.len() * 3);
    for [a, b, c] in indices {
        edges.push((*a as usize, *b as usize));
        edges.push((*b as usize, *c as usize));
        edges.push((*c as usize, *a as usize));
    }
    edges
}

fn boundary_edges(indices: &[EmbodiedTriangle]) -> Vec<(usize, usize)> {
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
    for (a, b) in mesh_edges(indices) {
        if a < vertex_count && b < vertex_count {
            adjacency[a].push(b);
            adjacency[b].push(a);
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThickBody<B> {
    pub base: B,
    pub thickness_map: ThicknessMap,
}

impl<B> ThickBody<B> {
    pub fn new(base: B, thickness_map: ThicknessMap) -> Self {
        Self {
            base,
            thickness_map,
        }
    }
}

impl<B> Embodied for ThickBody<B>
where
    B: Embodied<Vertex = EmbodiedPoint3, Index = EmbodiedTriangle, Bounds = EmbodiedBounds>,
{
    type Vertex = EmbodiedPoint3;
    type Index = EmbodiedTriangle;
    type Bounds = EmbodiedBounds;

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

    fn opt_resolution(&self) -> usize {
        self.base.opt_resolution()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{RevolutionAxis, RevolutionBody, Segment};

    fn make_body() -> RevolutionBody<1> {
        let segment = Segment::start_constant(1.0, 1.0).expect("segment");
        RevolutionBody::new([segment], RevolutionAxis::Y).expect("body")
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
        let thick = ThickBody::new(base, map);

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
        let thick = ThickBody::new(base, map);
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
}
