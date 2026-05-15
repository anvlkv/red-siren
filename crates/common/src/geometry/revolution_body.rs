use crate::geometry::Segment;
use crate::geometry::embodied::{Embodied, EmbodiedBounds, EmbodiedPoint3, EmbodiedTriangle};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Error type for RevolutionBody construction and operations.
#[derive(Debug, Clone, Error)]
pub enum RevolutionBodyError {
    #[error("Segments array is empty; must contain at least one segment")]
    EmptySegments,

    #[error("Segments are not monotonic; each segment must start where the previous ends")]
    NonMonotonicSegments,
}

/// The axis around which a profile is revolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevolutionAxis {
    /// Revolve around the X-axis.
    X,
    /// Revolve around the Y-axis.
    Y,
    /// Revolve around the Z-axis.
    Z,
}

/// A surface of revolution defined by N piecewise segments as a radius profile r(h).
///
/// The N segments together define r(h) — the radius at axial position h — over a
/// contiguous interval [h_min, h_max]. Revolving this profile around `axis` produces
/// a plain surface mesh via the [`Embodied`] trait.
///
/// # Generic Parameters
/// - `N`: compile-time number of segments in the profile.
///
/// # Fields
/// - `profile`: array of N segments defining the radius r(h).
/// - `axis`: the revolution axis (X, Y, or Z).
#[derive(Debug, Clone)]
pub struct RevolutionBody<const N: usize> {
    /// N segments defining the radius r(h) as a function of axial position h.
    pub profile: [Segment; N],
    /// The axis around which the profile is revolved.
    pub axis: RevolutionAxis,
}

// Manual Serialize implementation for RevolutionBody<N>
impl<const N: usize> Serialize for RevolutionBody<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("RevolutionBody", 2)?;
        let profile_vec: Vec<Segment> = self.profile.to_vec();
        state.serialize_field("profile", &profile_vec)?;
        state.serialize_field("axis", &self.axis)?;
        state.end()
    }
}

// Manual Deserialize implementation for RevolutionBody<N>
impl<'de, const N: usize> Deserialize<'de> for RevolutionBody<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct RevolutionBodyVisitor<const N: usize>;

        impl<'de, const N: usize> Visitor<'de> for RevolutionBodyVisitor<N> {
            type Value = RevolutionBody<N>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "struct RevolutionBody with {} profile segments",
                    N
                )
            }

            fn visit_map<V>(self, mut map: V) -> Result<RevolutionBody<N>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut profile: Option<Vec<Segment>> = None;
                let mut axis: Option<RevolutionAxis> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "profile" => {
                            if profile.is_some() {
                                return Err(de::Error::duplicate_field("profile"));
                            }
                            profile = Some(map.next_value()?);
                        }
                        "axis" => {
                            if axis.is_some() {
                                return Err(de::Error::duplicate_field("axis"));
                            }
                            axis = Some(map.next_value()?);
                        }
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                let profile_vec =
                    profile.ok_or_else(|| de::Error::missing_field("profile"))?;
                let axis = axis.ok_or_else(|| de::Error::missing_field("axis"))?;

                if profile_vec.len() != N {
                    return Err(de::Error::custom(format!(
                        "expected {} profile segments, got {}",
                        N,
                        profile_vec.len()
                    )));
                }

                let profile_array: [Segment; N] = profile_vec
                    .try_into()
                    .map_err(|_| de::Error::custom("failed to convert profile to array"))?;

                Ok(RevolutionBody {
                    profile: profile_array,
                    axis,
                })
            }
        }

        deserializer.deserialize_struct(
            "RevolutionBody",
            &["profile", "axis"],
            RevolutionBodyVisitor::<N>,
        )
    }
}

impl<const N: usize> RevolutionBody<N> {
    /// Construct a new RevolutionBody from a profile and a revolution axis.
    ///
    /// # Errors
    /// - [`RevolutionBodyError::EmptySegments`]: if N = 0.
    /// - [`RevolutionBodyError::NonMonotonicSegments`]: if segments don't form a contiguous chain.
    pub fn new(profile: [Segment; N], axis: RevolutionAxis) -> Result<Self, RevolutionBodyError> {
        if N == 0 {
            return Err(RevolutionBodyError::EmptySegments);
        }
        for i in 1..N {
            let prev_end = profile[i - 1].end;
            let curr_start = profile[i].start;
            if (prev_end - curr_start).abs() > 1e-10 {
                return Err(RevolutionBodyError::NonMonotonicSegments);
            }
        }
        Ok(RevolutionBody { profile, axis })
    }

    /// Get the axial range [min, max] of the profile.
    pub fn height_range(&self) -> (f64, f64) {
        (self.profile[0].start, self.profile[N - 1].end)
    }

    /// Get the number of segments in the profile.
    pub const fn segment_count(&self) -> usize {
        N
    }

    /// Generate normalized axial sample positions in [0, 1], distributed across segments
    /// proportionally by their `sampling_density`.
    pub fn profile_sample_positions(&self, total_budget: usize) -> Vec<f64> {
        if total_budget == 0 {
            return vec![];
        }

        let total_weight: f64 = self.profile.iter().map(|s| s.sampling_density).sum();
        if total_weight <= 0.0 {
            return (0..=total_budget)
                .map(|i| i as f64 / total_budget as f64)
                .collect();
        }

        let (min, max) = self.height_range();
        let span = max - min;
        let mut positions = Vec::new();
        let mut cumulative_u = 0.0;

        for seg in &self.profile {
            let seg_weight = seg.sampling_density / total_weight;
            let seg_samples = ((seg_weight * total_budget as f64).round() as usize).max(1);
            let seg_start_u = cumulative_u;
            let seg_end_u = cumulative_u + (seg.end - seg.start) / span;

            for i in 0..seg_samples {
                let seg_local_t = if seg_samples > 1 {
                    i as f64 / (seg_samples - 1) as f64
                } else {
                    0.0
                };
                let u = seg_start_u + seg_local_t * (seg_end_u - seg_start_u);
                positions.push(u.clamp(0.0, 1.0));
            }

            cumulative_u = seg_end_u;
        }

        positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        positions.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        positions
    }

    /// Sample the profile radius at a normalized axial position `u` in [0, 1].
    pub fn sample_profile(&self, u: f64) -> f64 {
        let (min, max) = self.height_range();
        let span = max - min;
        let t = min + u.clamp(0.0, 1.0) * span;

        for seg in &self.profile {
            if t >= seg.start && t <= seg.end {
                return seg.evaluate(t);
            }
        }

        self.profile[N - 1].evaluate(self.profile[N - 1].end)
    }

    /// Sample the profile radius and its derivative (dr/dt) at `u` in [0, 1].
    pub fn sample_profile_derivatives(&self, u: f64) -> (f64, f64) {
        let (min, max) = self.height_range();
        let span = max - min;
        let t = min + u.clamp(0.0, 1.0) * span;

        for seg in &self.profile {
            if t >= seg.start && t <= seg.end {
                return (seg.evaluate(t), seg.derivative_at(t));
            }
        }

        let last = &self.profile[N - 1];
        (last.evaluate(last.end), last.derivative_at(last.end))
    }

    /// Sample the profile as a sequence of (axial, radius) pairs.
    pub fn sampled_profile_points(&self, total_budget: usize) -> Vec<(f64, f64)> {
        let positions = self.profile_sample_positions(total_budget);
        let (min, max) = self.height_range();
        let span = max - min;
        positions
            .into_iter()
            .map(|u| (min + u * span, self.sample_profile(u)))
            .collect()
    }

    /// Maximum radius across the profile at `axial_budget` sampled positions.
    pub fn max_radius(&self, axial_budget: usize) -> f64 {
        self.sampled_profile_points(axial_budget)
            .iter()
            .map(|(_, r)| *r)
            .fold(0.0f64, f64::max)
    }
}

impl<const N: usize> Embodied for RevolutionBody<N> {
    type Vertex = EmbodiedPoint3;
    type Index = EmbodiedTriangle;
    type Bounds = EmbodiedBounds;

    /// Generate surface vertices by revolving the profile.
    ///
    /// `resolution` is used as both `theta_samples` (azimuthal divisions around the axis)
    /// and the axial sample budget, distributed across segments by their `sampling_density`.
    ///
    /// Layout: `points[j * radial_len + i]` where `j` is the theta index and `i` the axial
    /// index. Total vertex count = `theta_samples × radial_len`.
    fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex> {
        if resolution == 0 {
            return vec![];
        }

        let theta_samples = resolution;
        let sample_u = self.profile_sample_positions(resolution);
        if sample_u.is_empty() {
            return vec![];
        }

        let profile_pts = self.sampled_profile_points(resolution);
        let radial_len = sample_u.len();
        let theta_step = 2.0 * std::f64::consts::PI / theta_samples as f64;
        let mut points = Vec::with_capacity(theta_samples * radial_len);

        for j in 0..theta_samples {
            let theta = j as f64 * theta_step;
            let cos_t = theta.cos();
            let sin_t = theta.sin();

            for (i, &u) in sample_u.iter().enumerate() {
                let r = self.sample_profile(u);
                let h = profile_pts[i].0;

                points.push(match self.axis {
                    RevolutionAxis::X => EmbodiedPoint3::new(h, r * cos_t, r * sin_t),
                    RevolutionAxis::Y => EmbodiedPoint3::new(r * cos_t, h, r * sin_t),
                    RevolutionAxis::Z => EmbodiedPoint3::new(r * cos_t, r * sin_t, h),
                });
            }
        }

        points
    }

    /// Generate triangle indices consistent with `sample_points(resolution)`.
    ///
    /// Produces `2 × theta_samples × (radial_len − 1)` triangles with azimuthal seam wrapping.
    fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index> {
        if resolution < 2 {
            return vec![];
        }

        let theta_samples = resolution;
        let radial_positions = self.profile_sample_positions(resolution);
        let radial_len = radial_positions.len();
        if radial_len < 2 {
            return vec![];
        }

        let radial = radial_len as u32;
        let mut indices = Vec::with_capacity(2 * theta_samples * (radial_len - 1));

        for j in 0..theta_samples {
            let j_next = (j + 1) % theta_samples;
            let j0 = j as u32;
            let j1 = j_next as u32;

            for i in 0..(radial_len - 1) {
                let i0 = i as u32;
                let i1 = i0 + 1;

                let v00 = j0 * radial + i0;
                let v10 = j0 * radial + i1;
                let v01 = j1 * radial + i0;
                let v11 = j1 * radial + i1;

                indices.push([v00, v10, v11]);
                indices.push([v00, v11, v01]);
            }
        }

        indices
    }

    /// Compute the axis-aligned bounding box of the revolved surface.
    fn bounding_box(&self, resolution: usize) -> Self::Bounds {
        let points = self.sample_points(resolution.max(4));
        if points.is_empty() {
            let zero = EmbodiedPoint3::origin();
            return (zero, zero);
        }

        let mut min = points[0];
        let mut max = points[0];

        for p in &points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            min.z = min.z.min(p.z);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            max.z = max.z.max(p.z);
        }

        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_body_linear(end: f64, m: f64, b: f64, axis: RevolutionAxis) -> RevolutionBody<1> {
        let seg = Segment::start_line(end, m, b).expect("segment");
        RevolutionBody::new([seg], axis).expect("body")
    }

    fn make_body_constant(end: f64, value: f64) -> RevolutionBody<1> {
        let seg = Segment::start_constant(end, value).expect("segment");
        RevolutionBody::new([seg], RevolutionAxis::Y).expect("body")
    }

    // ── Construction ───────────────────────────────────────────────────────────

    #[test]
    fn test_construction_single_segment() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        assert_eq!(body.segment_count(), 1);
    }

    #[test]
    fn test_height_range() {
        let body = make_body_linear(2.0, 1.0, 0.0, RevolutionAxis::Y);
        assert_eq!(body.height_range(), (0.0, 2.0));
    }

    #[test]
    fn test_non_monotonic_segments() {
        let seg1 = Segment::start_line(1.0, 1.0, 0.0).expect("segment");
        let mut seg2 = Segment::start_line(2.0, 1.0, 0.0).expect("segment");
        seg2.start = 1.5; // intentional gap
        let result = RevolutionBody::new([seg1, seg2], RevolutionAxis::Y);
        assert!(matches!(result, Err(RevolutionBodyError::NonMonotonicSegments)));
    }

    #[test]
    fn test_monotonic_two_segments() {
        let seg1 = Segment::start_line(1.0, 1.0, 0.0).expect("segment");
        let mut seg2 = Segment::start_line(2.0, 1.0, 0.0).expect("segment");
        seg2.start = 1.0;
        let body = RevolutionBody::new([seg1, seg2], RevolutionAxis::Y).expect("body");
        assert_eq!(body.segment_count(), 2);
        assert_eq!(body.height_range(), (0.0, 2.0));
    }

    // ── Serialization ──────────────────────────────────────────────────────────

    #[test]
    fn test_serialization_roundtrip() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Z);
        let json = serde_json::to_string(&body).expect("serialize");
        let restored: RevolutionBody<1> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.axis, RevolutionAxis::Z);
        assert_eq!(restored.segment_count(), 1);
    }

    #[test]
    fn test_serialization_schema_has_profile_field() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        let json = serde_json::to_string(&body).expect("serialize");
        assert!(json.contains("\"profile\""), "must contain 'profile' key");
        assert!(!json.contains("outer_profile"), "must not contain legacy outer_profile");
        assert!(!json.contains("inner_profile"), "must not contain legacy inner_profile");
    }

    #[test]
    fn test_serialization_multi_segment_roundtrip() {
        let seg1 = Segment::start_line(1.0, 1.0, 0.0).expect("segment");
        let mut seg2 = Segment::start_line(2.0, 1.0, 0.0).expect("segment");
        seg2.start = 1.0;
        let body = RevolutionBody::new([seg1, seg2], RevolutionAxis::X).expect("body");
        let json = serde_json::to_string(&body).expect("serialize");
        let restored: RevolutionBody<2> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.axis, RevolutionAxis::X);
        assert_eq!(restored.height_range(), (0.0, 2.0));
    }

    // ── Profile sampling ───────────────────────────────────────────────────────

    #[test]
    fn test_profile_sample_positions_sorted_in_range() {
        let body = make_body_linear(1.0, 1.0, 1.0, RevolutionAxis::Y);
        let positions = body.profile_sample_positions(5);
        assert!(!positions.is_empty());
        assert!(positions[0] >= 0.0);
        assert!(*positions.last().unwrap() <= 1.0);
        for w in positions.windows(2) {
            assert!(w[0] <= w[1]);
        }
    }

    #[test]
    fn test_sample_profile_endpoints() {
        // r = 2t from t=0..1
        let body = make_body_linear(1.0, 2.0, 0.0, RevolutionAxis::Y);
        assert_eq!(body.sample_profile(0.0), 0.0);
        assert_eq!(body.sample_profile(1.0), 2.0);
    }

    #[test]
    fn test_sample_profile_mid() {
        // r = t from t=0..2, so u=0.5 → t=1 → r=1
        let body = make_body_linear(2.0, 1.0, 0.0, RevolutionAxis::Y);
        assert_eq!(body.sample_profile(0.5), 1.0);
    }

    #[test]
    fn test_sample_profile_derivatives() {
        let body = make_body_linear(2.0, 1.0, 0.0, RevolutionAxis::Y);
        let (r, dr) = body.sample_profile_derivatives(0.5);
        assert_eq!(r, 1.0);
        assert_eq!(dr, 1.0);
    }

    #[test]
    fn test_max_radius_constant() {
        let body = make_body_constant(1.0, 2.5);
        assert!((body.max_radius(5) - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_max_radius_empty_budget() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        assert_eq!(body.max_radius(0), 0.0);
    }

    // ── Embodied: sample_points ────────────────────────────────────────────────

    #[test]
    fn test_sample_points_count() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        let resolution = 8;
        let radial = body.profile_sample_positions(resolution).len();
        assert_eq!(body.sample_points(resolution).len(), resolution * radial);
    }

    #[test]
    fn test_sample_points_all_finite() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        for p in body.sample_points(6) {
            assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
        }
    }

    #[test]
    fn test_sample_points_empty_on_zero_resolution() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        assert!(body.sample_points(0).is_empty());
    }

    #[test]
    fn test_sample_points_y_axis_cylinder() {
        // constant r=1, Y-axis → every point is at distance 1 from Y-axis
        let body = make_body_constant(1.0, 1.0);
        for p in body.sample_points(8) {
            let d = (p.x * p.x + p.z * p.z).sqrt();
            assert!((d - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_sample_points_x_axis_cylinder() {
        let seg = Segment::start_constant(1.0, 1.0).expect("segment");
        let body = RevolutionBody::new([seg], RevolutionAxis::X).expect("body");
        for p in body.sample_points(8) {
            let d = (p.y * p.y + p.z * p.z).sqrt();
            assert!((d - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_sample_points_z_axis_cylinder() {
        let seg = Segment::start_constant(1.0, 1.0).expect("segment");
        let body = RevolutionBody::new([seg], RevolutionAxis::Z).expect("body");
        for p in body.sample_points(8) {
            let d = (p.x * p.x + p.y * p.y).sqrt();
            assert!((d - 1.0).abs() < 1e-10);
        }
    }

    // ── Embodied: mesh_indices ─────────────────────────────────────────────────

    #[test]
    fn test_mesh_indices_count() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        let resolution = 4;
        let radial = body.profile_sample_positions(resolution).len();
        let indices = body.mesh_indices(resolution);
        // 2 triangles per quad, (radial-1) quads per theta strip, theta_samples strips
        assert_eq!(indices.len(), 2 * resolution * (radial - 1));
    }

    #[test]
    fn test_mesh_indices_all_in_bounds() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        let resolution = 4;
        let vertex_count = body.sample_points(resolution).len() as u32;
        for [i0, i1, i2] in body.mesh_indices(resolution) {
            assert!(i0 < vertex_count);
            assert!(i1 < vertex_count);
            assert!(i2 < vertex_count);
        }
    }

    #[test]
    fn test_mesh_indices_empty_on_low_resolution() {
        let body = make_body_linear(1.0, 1.0, 0.0, RevolutionAxis::Y);
        assert!(body.mesh_indices(0).is_empty());
        assert!(body.mesh_indices(1).is_empty());
    }

    // ── Embodied: bounding_box ─────────────────────────────────────────────────

    #[test]
    fn test_bounding_box_y_axis_constant_radius() {
        let body = make_body_constant(1.0, 1.0);
        let (min, max) = body.bounding_box(8);
        // radius=1 around Y-axis → X and Z span [-1, 1]
        assert!(min.x < 0.0 && max.x > 0.0);
        assert!(min.y <= 0.0 && max.y >= 1.0);
        assert!(min.z < 0.0 && max.z > 0.0);
    }

    #[test]
    fn test_bounding_box_height_covers_profile_range() {
        let body = make_body_linear(2.0, 1.0, 0.0, RevolutionAxis::Y);
        let (min, max) = body.bounding_box(8);
        assert!(min.y <= 0.0);
        assert!(max.y >= 2.0);
    }

    #[test]
    fn test_bounding_box_x_axis_height_along_x() {
        let seg = Segment::start_constant(1.0, 1.0).expect("segment");
        let body = RevolutionBody::new([seg], RevolutionAxis::X).expect("body");
        let (min, max) = body.bounding_box(8);
        assert!(min.x <= 0.0 && max.x >= 1.0);
    }

    #[test]
    fn test_bounding_box_z_axis_height_along_z() {
        let seg = Segment::start_constant(1.0, 1.0).expect("segment");
        let body = RevolutionBody::new([seg], RevolutionAxis::Z).expect("body");
        let (min, max) = body.bounding_box(8);
        assert!(min.z <= 0.0 && max.z >= 1.0);
    }
}
