use std::cell::RefCell;

use fastrand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    body::meshable::{mesh_vertex_normals, EmbodiedVector3},
    EmbodiedBounds, EmbodiedPoint3, EmbodiedTriangle, Meshable,
};

const NORMAL_EPSILON: f64 = 1e-12;

/// Wraps a mesh provider and applies random per-vertex displacement along
/// each sampled vertex normal.
///
/// The displacement magnitude is sampled from
/// `[-deviation_scale.abs(), +deviation_scale.abs()]` on each `sample_points`
/// call, so output vertices are intentionally non-deterministic between calls.
#[derive(Clone, Debug)]
pub struct ImperfectMesh<
    M: Meshable<
        Vertex = EmbodiedPoint3,
        Index = EmbodiedTriangle,
        Bounds = EmbodiedBounds,
        Vector = EmbodiedVector3,
    >,
> {
    pub base: M,
    rng: RefCell<Rng>,
    deviation_scale: f64,
}

impl<
        M: Meshable<
            Vertex = EmbodiedPoint3,
            Index = EmbodiedTriangle,
            Bounds = EmbodiedBounds,
            Vector = EmbodiedVector3,
        >,
    > ImperfectMesh<M>
{
    pub fn new(base: M, deviation_scale: f64, rng: Rng) -> Self {
        ImperfectMesh {
            base,
            rng: RefCell::new(rng),
            deviation_scale,
        }
    }
}

impl<
        M: Meshable<
            Vertex = EmbodiedPoint3,
            Index = EmbodiedTriangle,
            Bounds = EmbodiedBounds,
            Vector = EmbodiedVector3,
        >,
    > Meshable for ImperfectMesh<M>
{
    type Vertex = EmbodiedPoint3;
    type Index = EmbodiedTriangle;
    type Bounds = EmbodiedBounds;
    type Vector = EmbodiedVector3;

    fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex> {
        let points = self.base.sample_points(resolution);
        if points.is_empty() {
            return points;
        }

        let scale = self.deviation_scale.abs();
        if scale <= f64::EPSILON {
            return points;
        }

        let indices = self.base.mesh_indices(resolution);
        if indices.is_empty() {
            return points;
        }

        let normals = mesh_vertex_normals(&points, &indices, NORMAL_EPSILON);

        let mut rng = self.rng.borrow_mut();
        points
            .into_iter()
            .enumerate()
            .map(|(idx, point)| {
                let Some(unit_normal) = normals
                    .get(idx)
                    .and_then(|normal| normal.try_normalize(NORMAL_EPSILON))
                else {
                    return point;
                };

                let displacement = rng.f64_inclusive() * (2.0 * scale) - scale;
                point + unit_normal * displacement
            })
            .collect()
    }

    fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index> {
        self.base.mesh_indices(resolution)
    }

    fn opt_resolution(&self) -> usize {
        self.base.opt_resolution()
    }
}

impl<
        M: Meshable<
                Vertex = EmbodiedPoint3,
                Index = EmbodiedTriangle,
                Bounds = EmbodiedBounds,
                Vector = EmbodiedVector3,
            > + Serialize,
    > Serialize for ImperfectMesh<M>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("ImperfectMesh", 3)?;
        state.serialize_field("base", &self.base)?;
        state.serialize_field("deviation_scale", &self.deviation_scale)?;
        state.serialize_field("rng_seed", &self.rng.borrow().get_seed())?;
        state.end()
    }
}

#[derive(Deserialize)]
struct ImperfectMeshSerde<M>
where
    M: Meshable<
        Vertex = EmbodiedPoint3,
        Index = EmbodiedTriangle,
        Bounds = EmbodiedBounds,
        Vector = EmbodiedVector3,
    >,
{
    base: M,
    deviation_scale: f64,
    rng_seed: u64,
}

impl<
        'de,
        M: Meshable<
                Vertex = EmbodiedPoint3,
                Index = EmbodiedTriangle,
                Bounds = EmbodiedBounds,
                Vector = EmbodiedVector3,
            > + Deserialize<'de>,
    > Deserialize<'de> for ImperfectMesh<M>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let serde_data = ImperfectMeshSerde::<M>::deserialize(deserializer)?;
        let mut rng = Rng::new();
        rng.seed(serde_data.rng_seed);

        Ok(Self {
            base: serde_data.base,
            rng: RefCell::new(rng),
            deviation_scale: serde_data.deviation_scale,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestMesh;

    impl Meshable for TestMesh {
        type Vertex = EmbodiedPoint3;
        type Index = EmbodiedTriangle;
        type Bounds = EmbodiedBounds;
        type Vector = EmbodiedVector3;

        fn opt_resolution(&self) -> usize {
            3
        }

        fn sample_points(&self, _resolution: usize) -> Vec<Self::Vertex> {
            vec![
                EmbodiedPoint3::new(0.0, 0.0, 0.0),
                EmbodiedPoint3::new(1.0, 0.0, 0.0),
                EmbodiedPoint3::new(0.0, 1.0, 0.0),
                EmbodiedPoint3::new(0.0, 0.0, 1.0),
            ]
        }

        fn mesh_indices(&self, _resolution: usize) -> Vec<Self::Index> {
            vec![[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]]
        }
    }

    #[test]
    fn seeded_rng_is_deterministic_for_first_sample() {
        let left = ImperfectMesh::new(TestMesh, 0.05, Rng::with_seed(42));
        let right = ImperfectMesh::new(TestMesh, 0.05, Rng::with_seed(42));

        let left_points = left.sample_points(8);
        let right_points = right.sample_points(8);

        assert_eq!(left_points, right_points);
    }

    #[test]
    fn serde_roundtrip_preserves_rng_progress() {
        let mesh = ImperfectMesh::new(TestMesh, 0.08, Rng::with_seed(7));

        let _ = mesh.sample_points(8);
        let _ = mesh.sample_points(8);

        let serialized = serde_json::to_string(&mesh).expect("serialize imperfect mesh");
        let restored: ImperfectMesh<TestMesh> =
            serde_json::from_str(&serialized).expect("deserialize imperfect mesh");

        let expected_next = mesh.sample_points(8);
        let restored_next = restored.sample_points(8);

        assert_eq!(expected_next, restored_next);
    }
}
