mod segment;

use mint::{Point2, Vector3};
use serde::{Deserialize, Serialize};

pub use segment::*;

const NUM_SEGMENTS: usize = 5;

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
/// Rotation body, models the geometry and material properties of a Bell, Gong, Cymbal, or other resonant node.
pub struct Node {
    /// Material properties of the node, used for mass and structural calculations.
    pub material: super::materials::Material,
    /// Segments defining the node's centerline geometry
    ///
    /// 1. Crown - top interior or central dome-like region
    /// 2. Upper body - upper half of the node's profile below the crown
    /// 3. Mid-body - broadest middle region of the node's profile
    /// 4. Shoulder - transition region between the mid-body and the rim, often with a noticeable change in curvature
    /// 5. Rim segment - outermost edge of the node's profile
    pub profile_segments: [Segment; NUM_SEGMENTS],
    /// Thickness of the node's shell in meters.
    ///
    /// The shell is defined as the area between the inner and outer profiles of the node.
    pub wall_thickness_m: [f64; NUM_SEGMENTS],
    /// Soundbow segment -  optional additional segment defining a soundbow feature.
    ///
    /// Essentially a dynamic rim thickness increment that can be applied to either the inner or outer profile at a specific location along the node's length, allowing for localized shaping of the soundbow feature without affecting the overall profile segments.
    ///
    /// 1. Inside soundbow - concave feature on the inner profile
    /// 2. Outside soundbow - convex feature on the outer profile
    pub soundbow_segments: [Option<Segment>; 2],
}

impl Node {
    /// Number of samples for rough calculations.
    pub const ROUGH_NUM_SAMPLES: usize = NUM_SEGMENTS * 4;

    /// Computes the overall size of the rotation body in meters, taking into account the maximum radius and height defined by the profile segments and wall thickness.
    pub fn size(&self, num_samples: usize) -> Vector3<f64> {
        todo!()
    }

    /// Tests if any given point is on the inside of the node's shell, meaning it is within the inner profile and not intersecting the material defined by the outer profile.
    pub fn is_on_the_inside(&self, point: Point2<f64>, num_samples: usize) -> bool {
        todo!()
    }

    /// Computes the thickness of the node's shell at a given normalized length `u` (0 to 1).
    pub fn thickness_at(&self, u: f64) -> f64 {
        let (outer, inner) = self.sample_at_u(u);
        let dx = outer.x - inner.x;
        let dy = outer.y - inner.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Computes the baseline point of the node at a given normalized length `u` (0 to 1), representing the centerline of the node's profile.
    fn baseline_at(&self, u: f64) -> Point2<f64> {
        todo!()
    }

    /// Samples the outer and inner profiles of the node at a given normalized length `u` (0 to 1).
    pub fn sample_at_u(&self, u: f64) -> (Point2<f64>, Point2<f64>) {
        todo!()
    }

    /// Computes the volume of the node's material in cubic meters.
    pub fn material_volume_m3(&self, num_samples: usize) -> f64 {
        todo!()
    }

    /// Computes the volume of the node's inner cavity in cubic meters, excluding material thickness.
    pub fn inner_volume_m3(&self, num_samples: usize) -> f64 {
        todo!()
    }

    /// Computes the mass of the node in kilograms, based on its material density and volume.
    pub fn mass_kg(&self, num_samples: usize) -> f64 {
        self.material.material_density_kg_per_m3 * self.material_volume_m3(num_samples)
    }
}
