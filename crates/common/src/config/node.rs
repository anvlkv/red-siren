use mint::Point2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct NodePhysics {
    /// Mass of the node in kg
    pub mass_kg: f64,
    /// Density of the material in kg/m^3
    pub material_density_kg_per_m3: f64,
    /// Thickness of the walls in meters
    pub wall_thickness_m: f64,
    /// Rim breadth
    pub rim_breadth: f64,
    /// R0 radius of the node's base ie the base along X axis in meters
    pub r0: f64,
    /// Base fillet radius of the node in meters
    pub r1: f64,
    /// R1 sweep angle in degrees (0 = no arc, 90 = quarter circle, 180 = half circle)
    pub r1_sweep: f64,
    /// Rim curve radius of the node in meters
    pub r2: f64,
    /// R2 sweep angle in degrees (0 = no arc, 90 = quarter circle, 180 = half circle)
    pub r2_sweep: f64,
    /// R1 Curves inward, _Like the inside of a bowl_?
    pub is_r1_concave: bool,
    /// R2 Curves inward, _Like the inside of a bowl_?
    pub is_r2_concave: bool,
    /// Length of the node base material in meters
    pub base_length_m: f64,
}

#[derive(Clone, Copy)]
struct PenState {
    point: Point2<f64>,
    tangent_angle: f64,
}

#[derive(Clone, Copy)]
enum PathSegment {
    Line {
        start: Point2<f64>,
        length: f64,
        tangent_angle: f64,
    },
    Arc {
        center: Point2<f64>,
        radial_start: Point2<f64>,
        radius: f64,
        sweep_cw: f64,
        tangent_angle_start: f64,
    },
}

impl PathSegment {
    fn length(self) -> f64 {
        match self {
            Self::Line { length, .. } => length,
            Self::Arc {
                radius, sweep_cw, ..
            } => radius * sweep_cw.abs(),
        }
    }

    fn eval(self, u: f64) -> (Point2<f64>, Point2<f64>) {
        let t = u.clamp(0.0, 1.0);
        match self {
            Self::Line {
                start,
                length,
                tangent_angle,
            } => {
                let dir = Point2 {
                    x: tangent_angle.cos(),
                    y: tangent_angle.sin(),
                };
                (
                    Point2 {
                        x: start.x + dir.x * length * t,
                        y: start.y + dir.y * length * t,
                    },
                    dir,
                )
            }
            Self::Arc {
                center,
                radial_start,
                sweep_cw,
                tangent_angle_start,
                ..
            } => {
                let rotated = NodePhysics::rotate_cw(radial_start, sweep_cw * t);
                let tangent_angle = tangent_angle_start + sweep_cw * t;
                (
                    Point2 {
                        x: center.x + rotated.x,
                        y: center.y + rotated.y,
                    },
                    Point2 {
                        x: tangent_angle.cos(),
                        y: tangent_angle.sin(),
                    },
                )
            }
        }
    }
}

impl NodePhysics {
    fn rim_segment_index(&self) -> Option<usize> {
        let segments = self.build_outer_segments();
        if segments.is_empty() {
            None
        } else {
            Some(segments.len() - 1)
        }
    }

    fn deg_to_rad(deg: f64) -> f64 {
        deg * std::f64::consts::PI / 180.0
    }

    // Rotate a vector by a clockwise angle in screen coordinates (y grows downward).
    fn rotate_cw(v: Point2<f64>, angle: f64) -> Point2<f64> {
        Point2 {
            x: v.x * angle.cos() - v.y * angle.sin(),
            y: v.x * angle.sin() + v.y * angle.cos(),
        }
    }

    fn arc_midpoint_x_for_turn(pen: PenState, radius: f64, sweep_rad: f64, turn_sign: f64) -> f64 {
        let center = Point2 {
            x: pen.point.x + turn_sign * radius * (-pen.tangent_angle.sin()),
            y: pen.point.y + turn_sign * radius * pen.tangent_angle.cos(),
        };
        let radial_start = Point2 {
            x: pen.point.x - center.x,
            y: pen.point.y - center.y,
        };
        let mid = Self::rotate_cw(radial_start, turn_sign * sweep_rad * 0.5);
        center.x + mid.x
    }

    // Choose the arc turn direction based on requested concavity relative to the axis (x=0).
    // concave=true -> inward bend (smaller x); concave=false -> outward bend (larger x).
    // When enforce_forward_end is true, prefer an end tangent with non-negative x component
    // so the following rim line does not flip backward.
    fn choose_turn_sign(
        pen: PenState,
        radius: f64,
        sweep_rad: f64,
        is_concave: bool,
        enforce_forward_end: bool,
    ) -> f64 {
        let x_plus = Self::arc_midpoint_x_for_turn(pen, radius, sweep_rad, 1.0);
        let x_minus = Self::arc_midpoint_x_for_turn(pen, radius, sweep_rad, -1.0);

        let preferred = if is_concave {
            if x_plus <= x_minus {
                1.0
            } else {
                -1.0
            }
        } else if x_plus >= x_minus {
            1.0
        } else {
            -1.0
        };

        if !enforce_forward_end {
            return preferred;
        }

        let other = -preferred;
        let preferred_end_x = (pen.tangent_angle + preferred * sweep_rad).cos();
        let other_end_x = (pen.tangent_angle + other * sweep_rad).cos();

        if preferred_end_x >= 0.0 {
            preferred
        } else if other_end_x >= 0.0 {
            other
        } else if other_end_x > preferred_end_x {
            other
        } else {
            preferred
        }
    }

    fn build_outer_segments(&self) -> Vec<PathSegment> {
        let mut segments = Vec::new();
        let mut pen = PenState {
            point: Point2 { x: 0.0, y: 0.0 },
            tangent_angle: 0.0,
        };

        // 1..2) Base line
        if self.r0 > 0.0 {
            segments.push(PathSegment::Line {
                start: pen.point,
                length: self.r0,
                tangent_angle: pen.tangent_angle,
            });
            pen.point = Point2 {
                x: pen.point.x + self.r0,
                y: pen.point.y,
            };
        }

        // 3) Arc r1
        let sweep1 = Self::deg_to_rad(self.r1_sweep.abs());
        if self.r1 > 0.0 && sweep1 > 0.0 && self.r1_sweep != 0.0 {
            let turn_sign = Self::choose_turn_sign(pen, self.r1, sweep1, self.is_r1_concave, false);
            // Negative r1_sweep flips the arc direction to point upward (toward -y)
            let effective_turn_sign = if self.r1_sweep < 0.0 {
                -turn_sign
            } else {
                turn_sign
            };
            let center = Point2 {
                x: pen.point.x + effective_turn_sign * self.r1 * (-pen.tangent_angle.sin()),
                y: pen.point.y + effective_turn_sign * self.r1 * pen.tangent_angle.cos(),
            };
            let radial_start = Point2 {
                x: pen.point.x - center.x,
                y: pen.point.y - center.y,
            };
            let sweep_cw = effective_turn_sign * sweep1;

            segments.push(PathSegment::Arc {
                center,
                radial_start,
                radius: self.r1,
                sweep_cw,
                tangent_angle_start: pen.tangent_angle,
            });

            let radial_end = Self::rotate_cw(radial_start, sweep_cw);
            pen.point = Point2 {
                x: center.x + radial_end.x,
                y: center.y + radial_end.y,
            };
            pen.tangent_angle += sweep_cw;
        }

        // 4..6) Middle tangent line (material length accounting)
        let sweep2 = Self::deg_to_rad(self.r2_sweep.abs());
        let used = self.r0.max(0.0)
            + self.r1.max(0.0) * sweep1.abs()
            + self.r2.max(0.0) * sweep2.abs()
            + self.rim_breadth.max(0.0);
        let middle_len = (self.base_length_m - used).max(0.0);
        if middle_len > 0.0 {
            segments.push(PathSegment::Line {
                start: pen.point,
                length: middle_len,
                tangent_angle: pen.tangent_angle,
            });
            pen.point = Point2 {
                x: pen.point.x + middle_len * pen.tangent_angle.cos(),
                y: pen.point.y + middle_len * pen.tangent_angle.sin(),
            };
        }

        // 7) Arc r2
        if self.r2 > 0.0 && sweep2 > 0.0 && self.r2_sweep != 0.0 {
            // R2 direction is explicit: concave bends inward, convex bends outward.
            // Keep deterministic sign here so shape presets can intentionally oppose.
            let turn_sign = if self.is_r2_concave { 1.0 } else { -1.0 };
            // Negative r2_sweep flips the arc direction to point upward (toward -y)
            let effective_turn_sign = if self.r2_sweep < 0.0 {
                -turn_sign
            } else {
                turn_sign
            };
            let center = Point2 {
                x: pen.point.x + effective_turn_sign * self.r2 * (-pen.tangent_angle.sin()),
                y: pen.point.y + effective_turn_sign * self.r2 * pen.tangent_angle.cos(),
            };
            let radial_start = Point2 {
                x: pen.point.x - center.x,
                y: pen.point.y - center.y,
            };
            let sweep_cw = effective_turn_sign * sweep2;

            segments.push(PathSegment::Arc {
                center,
                radial_start,
                radius: self.r2,
                sweep_cw,
                tangent_angle_start: pen.tangent_angle,
            });

            let radial_end = Self::rotate_cw(radial_start, sweep_cw);
            pen.point = Point2 {
                x: center.x + radial_end.x,
                y: center.y + radial_end.y,
            };
            pen.tangent_angle += sweep_cw;
        }

        // 8..9) Rim line
        let rim_len = self.rim_breadth.max(0.0);
        if rim_len > 0.0 {
            segments.push(PathSegment::Line {
                start: pen.point,
                length: rim_len,
                tangent_angle: pen.tangent_angle,
            });
        }

        segments
    }

    fn eval_outer_by_theta(&self, theta: f64) -> (Point2<f64>, Point2<f64>, usize, f64) {
        let segments = self.build_outer_segments();
        if segments.is_empty() {
            return (Point2 { x: 0.0, y: 0.0 }, Point2 { x: 1.0, y: 0.0 }, 0, 0.0);
        }

        let total_len: f64 = segments.iter().map(|seg| seg.length()).sum();
        if total_len <= 0.0 {
            return (Point2 { x: 0.0, y: 0.0 }, Point2 { x: 1.0, y: 0.0 }, 0, 0.0);
        }

        let t = if theta.is_finite() {
            theta.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut s = t * total_len;

        for (idx, segment) in segments.iter().enumerate() {
            let len = segment.length();
            if len <= 0.0 {
                continue;
            }
            if s <= len {
                let u = s / len;
                let (point, tangent) = segment.eval(u);
                return (point, tangent, idx, u);
            }
            s -= len;
        }

        let last_idx = segments.len() - 1;
        let (point, tangent) = segments[last_idx].eval(1.0);
        (point, tangent, last_idx, 1.0)
    }

    fn normalize_vector(v: Point2<f64>) -> Point2<f64> {
        let len = (v.x * v.x + v.y * v.y).sqrt();
        if len > 0.0 {
            Point2 {
                x: v.x / len,
                y: v.y / len,
            }
        } else {
            Point2 { x: 0.0, y: 0.0 }
        }
    }

    fn normal_candidates(tangent: Point2<f64>) -> (Point2<f64>, Point2<f64>) {
        let left = Self::normalize_vector(Point2 {
            x: -tangent.y,
            y: tangent.x,
        });
        let right = Self::normalize_vector(Point2 {
            x: tangent.y,
            y: -tangent.x,
        });

        (left, right)
    }

    fn inward_normal_for_tangent(
        tangent: Point2<f64>,
        previous_inner: Option<Point2<f64>>,
        outer: Point2<f64>,
    ) -> Point2<f64> {
        let (left, right) = Self::normal_candidates(tangent);
        let left_point = Point2 {
            x: outer.x + left.x,
            y: outer.y + left.y,
        };
        let right_point = Point2 {
            x: outer.x + right.x,
            y: outer.y + right.y,
        };

        if let Some(previous_inner) = previous_inner {
            let left_dist = (left_point.x - previous_inner.x).powi(2)
                + (left_point.y - previous_inner.y).powi(2);
            let right_dist = (right_point.x - previous_inner.x).powi(2)
                + (right_point.y - previous_inner.y).powi(2);
            if left_dist <= right_dist {
                left
            } else {
                right
            }
        } else if left.x <= right.x {
            left
        } else {
            right
        }
    }

    fn inner_from_outer(&self, outer: Point2<f64>, tangent: Point2<f64>) -> Point2<f64> {
        let n = Self::inward_normal_for_tangent(tangent, None, outer);

        let mut inner = Point2 {
            x: outer.x + n.x * self.wall_thickness_m.max(0.0),
            y: outer.y + n.y * self.wall_thickness_m.max(0.0),
        };
        if inner.x < 0.0 {
            inner.x = 0.0;
        }
        inner
    }

    pub fn points_at(&self, theta: f64) -> (Point2<f64>, Point2<f64>) {
        let (outer, tangent, seg_idx, seg_u) = self.eval_outer_by_theta(theta);
        let rim_scale = match self.rim_segment_index() {
            Some(rim_idx) if seg_idx == rim_idx => 1.0 - seg_u.clamp(0.0, 1.0),
            _ => 1.0,
        };
        let mut inner = self.inner_from_outer(outer, tangent);

        if rim_scale < 1.0 {
            // Taper shell thickness to zero across the rim so the very last rim point is sharp.
            inner = Point2 {
                x: outer.x + (inner.x - outer.x) * rim_scale,
                y: outer.y + (inner.y - outer.y) * rim_scale,
            };
        }

        (outer, inner)
    }

    pub fn thickness_at(&self, theta: f64) -> f64 {
        let (outer, inner) = self.points_at(theta);
        let dx = outer.x - inner.x;
        let dy = outer.y - inner.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns half profile outline of a node.
    ///
    /// Coordinate system:
    /// - `x`: radius (positive away from center axis)
    /// - `y`: height (increasing downward)
    pub fn outline(&self, num_samples_per_side: usize) -> Vec<Point2<f64>> {
        let samples = num_samples_per_side.max(1);
        let mut points = Vec::with_capacity((samples + 1) * 2 + 2);

        // Outer profile sampled forward by normalized material-length theta.
        for i in 0..=samples {
            let theta = i as f64 / samples as f64;
            let (outer, _) = self.points_at(theta);
            points.push(outer);
        }

        // Inner profile sampled backward to build a closed shell path.
        let mut inner_rev = Vec::with_capacity(samples + 1);
        let mut previous_inner = None;
        for i in (0..=samples).rev() {
            let theta = i as f64 / samples as f64;
            let (outer, tangent, seg_idx, seg_u) = self.eval_outer_by_theta(theta);
            let rim_scale = match self.rim_segment_index() {
                Some(rim_idx) if seg_idx == rim_idx => 1.0 - seg_u.clamp(0.0, 1.0),
                _ => 1.0,
            };

            let chosen_normal = Self::inward_normal_for_tangent(tangent, previous_inner, outer);
            let mut inner = Point2 {
                x: outer.x + chosen_normal.x * self.wall_thickness_m.max(0.0),
                y: outer.y + chosen_normal.y * self.wall_thickness_m.max(0.0),
            };

            if rim_scale < 1.0 {
                inner = Point2 {
                    x: outer.x + (inner.x - outer.x) * rim_scale,
                    y: outer.y + (inner.y - outer.y) * rim_scale,
                };
            }

            previous_inner = Some(inner);
            inner_rev.push(inner);
        }

        if !points.is_empty() && !inner_rev.is_empty() {
            let last = points[points.len() - 1];
            let first_inner = inner_rev[0];
            let dedup =
                (last.x - first_inner.x).abs() < 1e-9 && (last.y - first_inner.y).abs() < 1e-9;
            if dedup {
                points.extend_from_slice(&inner_rev[1..]);
            } else {
                points.extend_from_slice(&inner_rev);
            }
        }

        // Close from inner axis back to origin.
        let last_is_inner_axis = points
            .last()
            .map(|p| p.x.abs() < 1e-9 && (p.y - self.wall_thickness_m).abs() < 1e-6)
            .unwrap_or(false);
        if !last_is_inner_axis {
            points.push(Point2 {
                x: 0.0,
                y: self.wall_thickness_m,
            });
        }

        let last_is_origin = points
            .last()
            .map(|p| p.x.abs() < 1e-9 && p.y.abs() < 1e-9)
            .unwrap_or(false);
        if !last_is_origin {
            points.push(Point2 { x: 0.0, y: 0.0 });
        }

        points
    }
}

#[cfg(test)]
mod tests {
    use mint::Point2;

    use super::*;

    fn demo_node(is_r1_concave: bool, is_r2_concave: bool) -> NodePhysics {
        NodePhysics {
            mass_kg: 1.0,
            material_density_kg_per_m3: 1_000.0,
            wall_thickness_m: 0.05,
            rim_breadth: 0.1,
            r0: 0.25,
            r1: 0.2,
            r1_sweep: 35.0,
            r2: 0.18,
            r2_sweep: 35.0,
            is_r1_concave,
            is_r2_concave,
            base_length_m: 1.0,
        }
    }

    fn translated(points: &[Point2<f64>], dx: f64, dy: f64) -> Vec<Point2<f64>> {
        points
            .iter()
            .map(|p| Point2 {
                x: p.x + dx,
                y: p.y + dy,
            })
            .collect()
    }

    #[test]
    fn outline_snapshot_shape_variations() {
        let variants = [
            (
                "bowl_upwards",
                NodePhysics {
                    r0: 0.22,
                    r1: 0.24,
                    r2: 0.14,
                    r1_sweep: -45.0,
                    r2_sweep: -42.0,
                    rim_breadth: 0.10,
                    is_r1_concave: true,
                    is_r2_concave: false,
                    base_length_m: 2.04,
                    ..demo_node(false, false)
                },
            ),
            (
                "bell_downwards",
                NodePhysics {
                    r0: 0.24,
                    r1: 0.16,
                    r2: 0.20,
                    r1_sweep: 58.0,
                    r2_sweep: 52.0,
                    rim_breadth: 0.10,
                    is_r1_concave: false,
                    is_r2_concave: true,
                    base_length_m: 1.10,
                    ..demo_node(false, false)
                },
            ),
            (
                "bell_with_outward_rim",
                NodePhysics {
                    r0: 0.24,
                    r1: 0.16,
                    r2: 0.24,
                    r1_sweep: 56.0,
                    r2_sweep: 82.0,
                    rim_breadth: 0.14,
                    is_r1_concave: false,
                    is_r2_concave: false,
                    base_length_m: 1.16,
                    ..demo_node(false, false)
                },
            ),
            (
                "bowl_with_inward_rim",
                NodePhysics {
                    r0: 0.22,
                    r1: 0.22,
                    r2: 0.20,
                    r1_sweep: -44.0,
                    r2_sweep: -72.0,
                    rim_breadth: 0.10,
                    is_r1_concave: true,
                    is_r2_concave: true,
                    base_length_m: 1.06,
                    ..demo_node(false, false)
                },
            ),
            (
                "gong",
                NodePhysics {
                    wall_thickness_m: 0.04,
                    rim_breadth: 0.07,
                    r0: 0.28,
                    r1: 0.36,
                    r2: 0.34,
                    r1_sweep: 12.0,
                    r2_sweep: 14.0,
                    is_r1_concave: false,
                    is_r2_concave: false,
                    base_length_m: 1.14,
                    ..demo_node(false, false)
                },
            ),
            (
                "plate",
                NodePhysics {
                    wall_thickness_m: 0.025,
                    rim_breadth: 0.05,
                    r0: 0.32,
                    r1: 0.44,
                    r2: 0.42,
                    r1_sweep: -6.0,
                    r2_sweep: -8.0,
                    is_r1_concave: true,
                    is_r2_concave: false,
                    base_length_m: 1.20,
                    ..demo_node(false, false)
                },
            ),
        ];

        let mut all_paths: Vec<Vec<Point2<f64>>> = Vec::new();
        let mut cursor_x = 0.0;
        let spacing = 1.6;

        for (_, node) in variants {
            let outline = node.outline(20);
            all_paths.push(translated(&outline, cursor_x, 0.0));
            cursor_x += spacing;
        }

        let paths: Vec<&[Point2<f64>]> = all_paths.iter().map(Vec::as_slice).collect();
        crate::assert_paths_points_2d_snapshot!("node_outlines_shape_variations", &paths);
    }
}
