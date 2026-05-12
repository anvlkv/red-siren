use mint::Point2;

use super::{NodePhysics, EPS_COORD};

impl NodePhysics {
    pub fn sample_outer_inner_at(&self, u: f64) -> (Point2<f64>, Point2<f64>) {
        let rim_geom = self.rim_lip_geometry();
        let (mid, tangent, seg_idx, seg_u) = self.eval_outer_by_theta(u);
        let inward = self.inward_normal_for_theta(u, tangent, mid);
        let half_thickness = 0.5 * self.wall_thickness_m.max(0.0);
        let outer = Point2 {
            x: mid.x - inward.x * half_thickness,
            y: mid.y - inward.y * half_thickness,
        };
        let mut inner = Point2 {
            x: mid.x + inward.x * half_thickness,
            y: mid.y + inward.y * half_thickness,
        };
        if inner.x < 0.0 {
            inner.x = 0.0;
        }

        if let Some(geom) = rim_geom {
            if geom.seg_idx == seg_idx {
                let s = seg_u * geom.rim_len;
                return self.sample_rim_outer_inner(geom, s, u, true);
            }
        }

        (outer, inner)
    }

    pub fn thickness_at(&self, u: f64) -> f64 {
        let (outer, inner) = self.sample_outer_inner_at(u);
        let dx = outer.x - inner.x;
        let dy = outer.y - inner.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns half profile outline of a node.
    ///
    /// Coordinate system:
    /// - `x`: radius (positive away from center axis)
    /// - `y`: height (increasing downward)
    pub fn shell_outline(&self, num_samples_per_side: usize) -> Vec<Point2<f64>> {
        let samples = num_samples_per_side.max(1);
        let mut points = Vec::with_capacity((samples + 1) * 2 + 2);
        let rim_geom = self.rim_lip_geometry();

        // Outer profile sampled forward by normalized material-length theta.
        for i in 0..=samples {
            let u = Self::u_at_index(i, samples);
            let (outer, _) = self.sample_outer_inner_at(u);
            points.push(outer);
        }

        // Inner profile sampled backward to build a closed shell path.
        let mut inner_rev = Vec::with_capacity(samples + 1);
        for i in (0..=samples).rev() {
            let u = Self::u_at_index(i, samples);
            let (mid, tangent, seg_idx, seg_u) = self.eval_outer_by_theta(u);
            let inward = self.inward_normal_for_theta(u, tangent, mid);
            let half_thickness = 0.5 * self.wall_thickness_m.max(0.0);
            let outer = Point2 {
                x: mid.x - inward.x * half_thickness,
                y: mid.y - inward.y * half_thickness,
            };
            let mut inner = Point2 {
                x: mid.x + inward.x * half_thickness,
                y: mid.y + inward.y * half_thickness,
            };
            if inner.x < 0.0 {
                inner.x = 0.0;
            }

            let (_, inner) = if let Some(geom) = rim_geom {
                if geom.seg_idx == seg_idx {
                    let s = seg_u * geom.rim_len;
                    self.sample_rim_outer_inner(geom, s, u, false)
                } else {
                    (outer, inner)
                }
            } else {
                (outer, inner)
            };

            inner_rev.push(inner);
        }

        if !points.is_empty() && !inner_rev.is_empty() {
            let last = points[points.len() - 1];
            let first_inner = inner_rev[0];
            let dedup = (last.x - first_inner.x).abs() < EPS_COORD
                && (last.y - first_inner.y).abs() < EPS_COORD;
            if dedup {
                points.extend_from_slice(&inner_rev[1..]);
            } else {
                points.extend_from_slice(&inner_rev);
            }
        }

        // Close directly from inner start back to outer start; avoid axis-detour
        // closure segments that can self-intersect for steep profiles.
        if let Some(first) = points.first().copied() {
            let last_matches_first = points
                .last()
                .map(|p| (p.x - first.x).abs() < EPS_COORD && (p.y - first.y).abs() < EPS_COORD)
                .unwrap_or(false);
            if !last_matches_first {
                points.push(first);
            }
        }

        points
    }
}
