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
    /// Radius of the base segment in meters.
    pub base_radius_m: f64,
    /// Radius of the shoulder arc in meters.
    pub shoulder_radius_m: f64,
    /// Shoulder sweep angle in degrees (0 = no arc, 90 = quarter circle, 180 = half circle)
    pub shoulder_sweep_deg: f64,
    /// Radius of the rim arc in meters.
    pub rim_radius_m: f64,
    /// Rim sweep angle in degrees (0 = no arc, 90 = quarter circle, 180 = half circle)
    pub rim_sweep_deg: f64,
    /// Shoulder curves inward toward the axis when true.
    pub shoulder_curves_inward: bool,
    /// Rim curves inward toward the axis when true.
    pub rim_curves_inward: bool,
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

const EPS_COORD: f64 = 1e-9;

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
    fn u_at_index(index: usize, samples: usize) -> f64 {
        index as f64 / samples as f64
    }

    fn sweep_abs_deg_to_rad(deg: f64) -> f64 {
        Self::deg_to_rad(deg.abs())
    }

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

    fn arc_center_and_radial_start(
        pen: PenState,
        radius: f64,
        turn_sign: f64,
    ) -> (Point2<f64>, Point2<f64>) {
        let center = Point2 {
            x: pen.point.x + turn_sign * radius * (-pen.tangent_angle.sin()),
            y: pen.point.y + turn_sign * radius * pen.tangent_angle.cos(),
        };
        let radial_start = Point2 {
            x: pen.point.x - center.x,
            y: pen.point.y - center.y,
        };
        (center, radial_start)
    }

    fn append_arc_segment(
        segments: &mut Vec<PathSegment>,
        pen: &mut PenState,
        radius: f64,
        sweep_rad_abs: f64,
        effective_turn_sign: f64,
    ) {
        let (center, radial_start) =
            Self::arc_center_and_radial_start(*pen, radius, effective_turn_sign);
        let sweep_cw = effective_turn_sign * sweep_rad_abs;

        segments.push(PathSegment::Arc {
            center,
            radial_start,
            radius,
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

    fn arc_midpoint_x_for_turn(pen: PenState, radius: f64, sweep_rad: f64, turn_sign: f64) -> f64 {
        let (center, radial_start) = Self::arc_center_and_radial_start(pen, radius, turn_sign);
        let mid = Self::rotate_cw(radial_start, turn_sign * sweep_rad * 0.5);
        center.x + mid.x
    }

    fn rim_scale_for_segment(rim_idx: Option<usize>, seg_idx: usize, seg_u: f64) -> f64 {
        match rim_idx {
            Some(idx) if seg_idx == idx => 1.0 - seg_u.clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    fn apply_rim_taper(outer: Point2<f64>, inner: Point2<f64>, rim_scale: f64) -> Point2<f64> {
        if rim_scale >= 1.0 {
            return inner;
        }

        Point2 {
            x: outer.x + (inner.x - outer.x) * rim_scale,
            y: outer.y + (inner.y - outer.y) * rim_scale,
        }
    }

    fn compute_inner_point(
        &self,
        outer: Point2<f64>,
        tangent: Point2<f64>,
        previous_inner: Option<Point2<f64>>,
        rim_scale: f64,
        clamp_x_to_axis: bool,
    ) -> Point2<f64> {
        let normal = Self::inward_normal_for_tangent(tangent, previous_inner, outer);
        let thickness = self.wall_thickness_m.max(0.0);
        let mut inner = Point2 {
            x: outer.x + normal.x * thickness,
            y: outer.y + normal.y * thickness,
        };

        if clamp_x_to_axis && inner.x < 0.0 {
            inner.x = 0.0;
        }

        Self::apply_rim_taper(outer, inner, rim_scale)
    }

    // Choose the arc turn direction based on requested concavity relative to the axis (x=0).
    // concave=true -> inward bend (smaller x); concave=false -> outward bend (larger x).
    // When enforce_forward_end is true, prefer an end tangent with non-negative x component
    // so the following rim line does not flip backward.
    fn choose_arc_turn_sign(
        pen: PenState,
        radius: f64,
        sweep_rad: f64,
        curves_inward: bool,
        enforce_forward_end: bool,
    ) -> f64 {
        let x_plus = Self::arc_midpoint_x_for_turn(pen, radius, sweep_rad, 1.0);
        let x_minus = Self::arc_midpoint_x_for_turn(pen, radius, sweep_rad, -1.0);

        let preferred = if curves_inward {
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

        // Base line.
        if self.base_radius_m > 0.0 {
            segments.push(PathSegment::Line {
                start: pen.point,
                length: self.base_radius_m,
                tangent_angle: pen.tangent_angle,
            });
            pen.point = Point2 {
                x: pen.point.x + self.base_radius_m,
                y: pen.point.y,
            };
        }

        // Shoulder arc.
        let shoulder_sweep = Self::sweep_abs_deg_to_rad(self.shoulder_sweep_deg);
        if self.shoulder_radius_m > 0.0 && shoulder_sweep > 0.0 {
            // `effective_turn_sign` is negated for negative sweeps, so invert the
            // preference before selection to preserve the requested concavity.
            let shoulder_curves_inward = if self.shoulder_sweep_deg < 0.0 {
                !self.shoulder_curves_inward
            } else {
                self.shoulder_curves_inward
            };
            let turn_sign = Self::choose_arc_turn_sign(
                pen,
                self.shoulder_radius_m,
                shoulder_sweep,
                shoulder_curves_inward,
                false,
            );
            // Negative sweeps flip the arc direction in screen space.
            let effective_turn_sign = if self.shoulder_sweep_deg < 0.0 {
                -turn_sign
            } else {
                turn_sign
            };
            Self::append_arc_segment(
                &mut segments,
                &mut pen,
                self.shoulder_radius_m,
                shoulder_sweep,
                effective_turn_sign,
            );
        }

        // 4..6) Middle tangent line (material length accounting)
        let rim_sweep = Self::sweep_abs_deg_to_rad(self.rim_sweep_deg);
        let used = self.base_radius_m.max(0.0)
            + self.shoulder_radius_m.max(0.0) * shoulder_sweep
            + self.rim_radius_m.max(0.0) * rim_sweep
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

        // Rim arc.
        if self.rim_radius_m > 0.0 && rim_sweep > 0.0 {
            // `effective_turn_sign` is negated for negative sweeps, so invert the
            // preference before selection to preserve the requested concavity.
            let rim_curves_inward = if self.rim_sweep_deg < 0.0 {
                !self.rim_curves_inward
            } else {
                self.rim_curves_inward
            };
            let turn_sign = Self::choose_arc_turn_sign(
                pen,
                self.rim_radius_m,
                rim_sweep,
                rim_curves_inward,
                false,
            );
            // Negative sweeps flip the arc direction in screen space.
            let effective_turn_sign = if self.rim_sweep_deg < 0.0 {
                -turn_sign
            } else {
                turn_sign
            };
            Self::append_arc_segment(
                &mut segments,
                &mut pen,
                self.rim_radius_m,
                rim_sweep,
                effective_turn_sign,
            );
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

    pub fn sample_outer_inner_at(&self, u: f64) -> (Point2<f64>, Point2<f64>) {
        let rim_idx = self.rim_segment_index();
        let (outer, tangent, seg_idx, seg_u) = self.eval_outer_by_theta(u);
        let rim_scale = Self::rim_scale_for_segment(rim_idx, seg_idx, seg_u);
        let inner = self.compute_inner_point(outer, tangent, None, rim_scale, true);

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
        let rim_idx = self.rim_segment_index();

        // Outer profile sampled forward by normalized material-length theta.
        for i in 0..=samples {
            let u = Self::u_at_index(i, samples);
            let (outer, _) = self.sample_outer_inner_at(u);
            points.push(outer);
        }

        // Inner profile sampled backward to build a closed shell path.
        let mut inner_rev = Vec::with_capacity(samples + 1);
        let mut previous_inner = None;
        for i in (0..=samples).rev() {
            let u = Self::u_at_index(i, samples);
            let (outer, tangent, seg_idx, seg_u) = self.eval_outer_by_theta(u);
            let rim_scale = Self::rim_scale_for_segment(rim_idx, seg_idx, seg_u);
            let inner = self.compute_inner_point(outer, tangent, previous_inner, rim_scale, false);

            previous_inner = Some(inner);
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

        // Project the final inner point to the axis at its current height,
        // then close to origin. Using a fixed y here can create visible notches
        // for steep presets where the inner path ends far from wall_thickness_m.
        if let Some(last) = points.last().copied() {
            if last.x.abs() >= EPS_COORD {
                points.push(Point2 { x: 0.0, y: last.y });
            }
        }

        let last_is_origin = points
            .last()
            .map(|p| p.x.abs() < EPS_COORD && p.y.abs() < EPS_COORD)
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

    fn demo_node(shoulder_curves_inward: bool, rim_curves_inward: bool) -> NodePhysics {
        NodePhysics {
            mass_kg: 1.0,
            material_density_kg_per_m3: 1_000.0,
            wall_thickness_m: 0.05,
            rim_breadth: 0.1,
            base_radius_m: 0.25,
            shoulder_radius_m: 0.2,
            shoulder_sweep_deg: 35.0,
            rim_radius_m: 0.18,
            rim_sweep_deg: 35.0,
            shoulder_curves_inward,
            rim_curves_inward,
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
                    base_radius_m: 0.22,
                    shoulder_radius_m: 0.24,
                    rim_radius_m: 0.14,
                    shoulder_sweep_deg: -45.0,
                    rim_sweep_deg: -42.0,
                    rim_breadth: 0.10,
                    shoulder_curves_inward: true,
                    rim_curves_inward: false,
                    base_length_m: 2.04,
                    ..demo_node(false, false)
                },
            ),
            (
                "bell_downwards",
                NodePhysics {
                    base_radius_m: 0.24,
                    shoulder_radius_m: 0.16,
                    rim_radius_m: 0.20,
                    shoulder_sweep_deg: 58.0,
                    rim_sweep_deg: 52.0,
                    rim_breadth: 0.10,
                    shoulder_curves_inward: false,
                    rim_curves_inward: true,
                    base_length_m: 1.10,
                    ..demo_node(false, false)
                },
            ),
            (
                "bell_with_outward_rim",
                NodePhysics {
                    base_radius_m: 0.24,
                    shoulder_radius_m: 0.16,
                    rim_radius_m: 0.24,
                    shoulder_sweep_deg: 56.0,
                    rim_sweep_deg: 82.0,
                    rim_breadth: 0.14,
                    shoulder_curves_inward: false,
                    rim_curves_inward: false,
                    base_length_m: 1.16,
                    ..demo_node(false, false)
                },
            ),
            (
                "bowl_with_inward_rim",
                NodePhysics {
                    base_radius_m: 0.22,
                    shoulder_radius_m: 0.22,
                    rim_radius_m: 0.20,
                    shoulder_sweep_deg: -44.0,
                    rim_sweep_deg: -72.0,
                    rim_breadth: 0.10,
                    shoulder_curves_inward: true,
                    rim_curves_inward: true,
                    base_length_m: 1.06,
                    ..demo_node(false, false)
                },
            ),
            (
                "gong",
                NodePhysics {
                    wall_thickness_m: 0.04,
                    rim_breadth: 0.07,
                    base_radius_m: 0.28,
                    shoulder_radius_m: 0.36,
                    rim_radius_m: 0.34,
                    shoulder_sweep_deg: 12.0,
                    rim_sweep_deg: 14.0,
                    shoulder_curves_inward: false,
                    rim_curves_inward: false,
                    base_length_m: 1.14,
                    ..demo_node(false, false)
                },
            ),
            (
                "plate",
                NodePhysics {
                    wall_thickness_m: 0.025,
                    rim_breadth: 0.05,
                    base_radius_m: 0.32,
                    shoulder_radius_m: 0.44,
                    rim_radius_m: 0.42,
                    shoulder_sweep_deg: -6.0,
                    rim_sweep_deg: -8.0,
                    shoulder_curves_inward: true,
                    rim_curves_inward: false,
                    base_length_m: 1.20,
                    ..demo_node(false, false)
                },
            ),
        ];

        let mut all_paths: Vec<Vec<Point2<f64>>> = Vec::new();
        let mut cursor_x = 0.0;
        let spacing = 1.6;

        for (_, node) in variants {
            let outline = node.shell_outline(20);
            all_paths.push(translated(&outline, cursor_x, 0.0));
            cursor_x += spacing;
        }

        let paths: Vec<&[Point2<f64>]> = all_paths.iter().map(Vec::as_slice).collect();
        crate::assert_paths_points_2d_snapshot!("node_outlines_shape_variations", &paths);
    }
}
