pub mod materials;
mod memo_mesh;
mod meshable;
mod revolution_mesh;
mod segment;
mod thick_mesh;

use materials::Material;
pub use memo_mesh::MemoMesh;
pub use meshable::{
    mesh_surface_area_m2, mesh_vertex_normals, nearest_vertex_index, EmbodiedBounds,
    EmbodiedPoint3, EmbodiedTriangle, Meshable, SurfaceMesh,
};
pub use revolution_mesh::{RevolutionAxis, RevolutionBodyError, RevolutionMesh};
pub use segment::*;
use serde::{Deserialize, Serialize};
pub use thick_mesh::{
    thickness_map_from_axial_samples, ThickMesh, ThicknessMap, ThicknessMapPoint,
};

use crate::body::meshable::EmbodiedVector3;

const MASS_PROPERTIES_EPSILON: f64 = 1e-12;

fn finite_point3(p: EmbodiedPoint3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

/// Line between two points
pub type Line = (
    nalgebra::geometry::Point2<i64>,
    nalgebra::geometry::Point2<i64>,
);

/// Rectangle from start of coordinates
///
/// - start `nalgebra::geometry::Point2<i64>`
/// - size `nalgebra::base::Vector2<i64>`
pub type Rect = (
    nalgebra::geometry::Point2<i64>,
    nalgebra::base::Vector2<i64>,
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body<
    M: Meshable<
        Vertex = EmbodiedPoint3,
        Index = EmbodiedTriangle,
        Bounds = EmbodiedBounds,
        Vector = EmbodiedVector3,
    >,
> {
    pub meshable: M,
    pub material: Material,
}

impl<
        M: Meshable<
            Vertex = EmbodiedPoint3,
            Index = EmbodiedTriangle,
            Bounds = EmbodiedBounds,
            Vector = EmbodiedVector3,
        >,
    > Body<M>
{
    pub fn new(meshable: M, material: Material) -> Self {
        Self { meshable, material }
    }

    pub fn mass_kg(&self, resolution: usize) -> f64 {
        let volume_m3 = self.meshable.material_volume_m3(resolution);
        volume_m3 * self.material.reference_density_kg_per_m3
    }

    pub fn center_of_mass(&self, resolution: usize) -> Option<EmbodiedPoint3> {
        let mesh = self.meshable.surface_mesh_data(resolution.max(3));
        let points = mesh.vertices;
        let indices = mesh.indices;
        if points.is_empty() || indices.is_empty() {
            return None;
        }

        let mut signed_volume_sum = 0.0;
        let mut weighted_centroid_sum = nalgebra::Vector3::zeros();
        let mut abs_volume_sum = 0.0;
        let mut weighted_centroid_abs_sum = nalgebra::Vector3::zeros();

        for [a, b, c] in indices {
            let (a, b, c) = (a as usize, b as usize, c as usize);
            if a >= points.len() || b >= points.len() || c >= points.len() {
                continue;
            }

            let p0 = points[a];
            let p1 = points[b];
            let p2 = points[c];
            if !(finite_point3(p0) && finite_point3(p1) && finite_point3(p2)) {
                continue;
            }

            let signed_volume = p0.coords.dot(&p1.coords.cross(&p2.coords)) / 6.0;
            if !signed_volume.is_finite() {
                continue;
            }

            let tetra_centroid = (p0.coords + p1.coords + p2.coords) / 4.0;

            signed_volume_sum += signed_volume;
            weighted_centroid_sum += tetra_centroid * signed_volume;

            let abs_volume = signed_volume.abs();
            abs_volume_sum += abs_volume;
            weighted_centroid_abs_sum += tetra_centroid * abs_volume;
        }

        let centroid = if signed_volume_sum.abs() > MASS_PROPERTIES_EPSILON {
            weighted_centroid_sum / signed_volume_sum
        } else if abs_volume_sum > MASS_PROPERTIES_EPSILON {
            weighted_centroid_abs_sum / abs_volume_sum
        } else {
            return None;
        };

        if !(centroid.x.is_finite() && centroid.y.is_finite() && centroid.z.is_finite()) {
            return None;
        }

        Some(EmbodiedPoint3::from(centroid))
    }

    pub fn inertia_tensor(&self, resolution: usize) -> Option<nalgebra::Matrix3<f64>> {
        let density = self.material.reference_density_kg_per_m3;
        if !density.is_finite() || density <= 0.0 {
            return None;
        }

        let center_of_mass = self.center_of_mass(resolution.max(3))?;
        let mesh = self.meshable.surface_mesh_data(resolution.max(3));
        let points = mesh.vertices;
        let indices = mesh.indices;
        if points.is_empty() || indices.is_empty() {
            return None;
        }

        let mut inertia = nalgebra::Matrix3::zeros();
        let mut mass_sum = 0.0;

        for [a, b, c] in indices {
            let (a, b, c) = (a as usize, b as usize, c as usize);
            if a >= points.len() || b >= points.len() || c >= points.len() {
                continue;
            }

            let p0 = points[a];
            let p1 = points[b];
            let p2 = points[c];
            if !(finite_point3(p0) && finite_point3(p1) && finite_point3(p2)) {
                continue;
            }

            let signed_volume = p0.coords.dot(&p1.coords.cross(&p2.coords)) / 6.0;
            let tetra_volume = signed_volume.abs();
            if !tetra_volume.is_finite() || tetra_volume <= MASS_PROPERTIES_EPSILON {
                continue;
            }

            let tetra_mass = density * tetra_volume;
            if !tetra_mass.is_finite() || tetra_mass <= MASS_PROPERTIES_EPSILON {
                continue;
            }

            let tetra_centroid = EmbodiedPoint3::from((p0.coords + p1.coords + p2.coords) / 4.0);
            let r = tetra_centroid - center_of_mass;
            let rr_t = r * r.transpose();
            let r2 = r.dot(&r);
            let contribution = (nalgebra::Matrix3::identity() * r2 - rr_t) * tetra_mass;
            inertia += contribution;
            mass_sum += tetra_mass;
        }

        if mass_sum <= MASS_PROPERTIES_EPSILON {
            return None;
        }

        // Keep the tensor explicitly symmetric and robust to tiny floating noise.
        let mut sym = (inertia + inertia.transpose()) * 0.5;
        for i in 0..3 {
            if sym[(i, i)] < 0.0 && sym[(i, i)] > -MASS_PROPERTIES_EPSILON {
                sym[(i, i)] = 0.0;
            }
        }

        if sym.iter().all(|v| v.is_finite()) {
            Some(sym)
        } else {
            None
        }
    }

    pub fn surface_mesh(&self, resolution: usize) -> (Vec<EmbodiedPoint3>, Vec<EmbodiedTriangle>) {
        self.meshable.surface_mesh_data(resolution).into_parts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct UnitCube;

    impl Meshable for UnitCube {
        type Vertex = EmbodiedPoint3;
        type Index = EmbodiedTriangle;
        type Bounds = EmbodiedBounds;
        type Vector = EmbodiedVector3;

        fn sample_points(&self, _resolution: usize) -> Vec<Self::Vertex> {
            vec![
                EmbodiedPoint3::new(0.0, 0.0, 0.0),
                EmbodiedPoint3::new(1.0, 0.0, 0.0),
                EmbodiedPoint3::new(1.0, 1.0, 0.0),
                EmbodiedPoint3::new(0.0, 1.0, 0.0),
                EmbodiedPoint3::new(0.0, 0.0, 1.0),
                EmbodiedPoint3::new(1.0, 0.0, 1.0),
                EmbodiedPoint3::new(1.0, 1.0, 1.0),
                EmbodiedPoint3::new(0.0, 1.0, 1.0),
            ]
        }

        fn mesh_indices(&self, _resolution: usize) -> Vec<Self::Index> {
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [3, 7, 6],
                [3, 6, 2],
                [0, 4, 7],
                [0, 7, 3],
                [1, 2, 6],
                [1, 6, 5],
            ]
        }

        fn bounding_box(&self, _resolution: usize) -> Self::Bounds {
            (
                EmbodiedPoint3::new(0.0, 0.0, 0.0),
                EmbodiedPoint3::new(1.0, 1.0, 1.0),
            )
        }

        fn material_volume_m3(&self, _resolution: usize) -> f64 {
            1.0
        }

        fn cavity_volume_m3(&self, _resolution: usize) -> Option<f64> {
            None
        }

        fn surface_normal_at_vertex(
            &self,
            _resolution: usize,
            _vertex_index: usize,
        ) -> Option<Self::Vector> {
            None
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
            8
        }
    }

    fn steel() -> Material {
        Material {
            id: "BODY_STEEL_TEST".to_string(),
            reference_density_kg_per_m3: 7800.0,
            poisson_ratio: 0.29,
            reference_youngs_modulus_mpa: 200_000.0,
            reference_temperature_c: 20.0,
            linear_thermal_expansion_per_c: 12.0e-6,
            dln_e_dtemp_per_c: -4.0e-4,
        }
    }

    #[test]
    fn center_of_mass_matches_unit_cube_centroid() {
        let body = Body::new(UnitCube, steel());
        let center = body.center_of_mass(16).expect("center");

        assert!((center.x - 0.5).abs() < 1e-9);
        assert!((center.y - 0.5).abs() < 1e-9);
        assert!((center.z - 0.5).abs() < 1e-9);
    }

    #[test]
    fn inertia_tensor_is_finite_symmetric_and_positive() {
        let body = Body::new(UnitCube, steel());
        let inertia = body.inertia_tensor(16).expect("inertia tensor");

        for value in inertia.iter() {
            assert!(value.is_finite());
        }

        assert!((inertia[(0, 1)] - inertia[(1, 0)]).abs() < 1e-9);
        assert!((inertia[(0, 2)] - inertia[(2, 0)]).abs() < 1e-9);
        assert!((inertia[(1, 2)] - inertia[(2, 1)]).abs() < 1e-9);

        assert!(inertia[(0, 0)] >= 0.0);
        assert!(inertia[(1, 1)] >= 0.0);
        assert!(inertia[(2, 2)] >= 0.0);
    }
}
