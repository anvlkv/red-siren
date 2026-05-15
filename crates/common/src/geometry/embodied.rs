use nalgebra::Point3;

/// A generic surface meshing interface for any meshable geometry.
pub trait Embodied {
    type Vertex;
    type Index;
    type Bounds;

    /// Sample surface vertices given a resolution hint.
    fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex>;

    /// Generate triangle index topology consistent with the vertices from `sample_points`.
    fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index>;

    /// Compute the axis-aligned bounding box of the surface.
    fn bounding_box(&self, resolution: usize) -> Self::Bounds;

    /// Optimal resolution hint for sampling this geometry, if any. This can be used by downstream code to avoid unnecessary sampling at very high resolutions.
    fn opt_resolution(&self) -> usize;
}

/// Default mesh output aliases used by RevolutionBody.
pub type EmbodiedPoint3 = Point3<f64>;
pub type EmbodiedTriangle = [u32; 3];
pub type EmbodiedBounds = (Point3<f64>, Point3<f64>);
