use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LocalConvexity {
    Convex,
    Concave,
    Flat,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MidpointSample {
    pub point: Point2<f64>,
    pub tangent: Point2<f64>,
    pub convexity: LocalConvexity,
}

impl NodePhysics {
    pub(super) fn u_at_index(index: usize, samples: usize) -> f64 {
        index as f64 / samples as f64
    }

    pub(super) fn classify_convexity(
        prev_tangent: Point2<f64>,
        next_tangent: Point2<f64>,
    ) -> LocalConvexity {
        let turn = prev_tangent.x * next_tangent.y - prev_tangent.y * next_tangent.x;
        if turn > EPS_COORD {
            LocalConvexity::Concave
        } else if turn < -EPS_COORD {
            LocalConvexity::Convex
        } else {
            LocalConvexity::Flat
        }
    }

    // Midpoint-path rewrite entrypoint: sample a stable centerline proxy with local convexity.
    pub(super) fn midpoint_sample_at(&self, u: f64) -> MidpointSample {
        let (point, tangent, _, _) = self.eval_outer_by_theta(u.clamp(0.0, 1.0));
        let delta = 1.0 / 1024.0;
        let up = (u - delta).clamp(0.0, 1.0);
        let un = (u + delta).clamp(0.0, 1.0);
        let (_, prev_tangent, _, _) = self.eval_outer_by_theta(up);
        let (_, next_tangent, _, _) = self.eval_outer_by_theta(un);

        MidpointSample {
            point,
            tangent,
            convexity: Self::classify_convexity(prev_tangent, next_tangent),
        }
    }
}
