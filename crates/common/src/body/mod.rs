pub mod materials;
mod memo_mesh;
mod meshable;
mod revolution_mesh;
mod segment;
mod thick_mesh;

use materials::Material;
pub use memo_mesh::MemoMesh;
pub use meshable::{EmbodiedBounds, EmbodiedPoint3, EmbodiedTriangle, Meshable};
pub use revolution_mesh::{RevolutionAxis, RevolutionBodyError, RevolutionMesh};
pub use segment::*;
use serde::{Deserialize, Serialize};
pub use thick_mesh::{
    thickness_map_from_axial_samples, ThickMesh, ThicknessMap, ThicknessMapPoint,
};

use crate::body::meshable::EmbodiedVector3;

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
        volume_m3 * self.material.density_kg_per_m3
    }

    pub fn center_of_mass(&self, resolution: usize) -> Option<EmbodiedPoint3> {
        todo!()
    }

    pub fn inertia_tensor(&self, resolution: usize) -> Option<nalgebra::Matrix3<f64>> {
        todo!()
    }

    pub fn surface_mesh(&self, resolution: usize) -> (Vec<EmbodiedPoint3>, Vec<EmbodiedTriangle>) {
        let vertices = self.meshable.sample_points(resolution);
        let indices = self.meshable.mesh_indices(resolution);
        (vertices, indices)
    }
}
