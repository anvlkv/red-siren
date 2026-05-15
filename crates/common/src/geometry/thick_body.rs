use crate::geometry::embodied::{Embodied, EmbodiedBounds, EmbodiedPoint3, EmbodiedTriangle};
use serde::{Deserialize, Serialize};

const DIRECTION_EPSILON: f64 = 1e-9;

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

        let (min, max) = self.base.bounding_box(resolution.max(2));
        let center = EmbodiedPoint3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (min.z + max.z) * 0.5,
        );

        let mut points = Vec::with_capacity(base_points.len() * 2);

        for (index, base_point) in base_points.iter().enumerate() {
            let (face_thickness, backface_thickness) = self.thickness_map.sample(index);

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

        let mut indices = Vec::with_capacity(base_indices.len() * 2);
        for [a, b, c] in &base_indices {
            indices.push([a * 2, b * 2, c * 2]);
            indices.push([c * 2 + 1, b * 2 + 1, a * 2 + 1]);
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
    fn thick_body_emits_two_layers_and_indices() {
        let base = make_body();
        let resolution = 8;
        let base_points = base.sample_points(resolution);
        let base_indices = base.mesh_indices(resolution);

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
        assert_eq!(indices.len(), base_indices.len() * 2);

        let vertex_count = points.len() as u32;
        for [a, b, c] in indices {
            assert!(a < vertex_count);
            assert!(b < vertex_count);
            assert!(c < vertex_count);
        }
    }
}
