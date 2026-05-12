use mint::Point2;
use serde::{Deserialize, Serialize};

mod geometry;
mod sampling;
mod volume;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct NodePhysics {
    /// Density of the material in kg/m^3.
    ///
    /// Node mass is derived from this density and the computed material volume.
    pub material_density_kg_per_m3: f64,
    /// Thickness of the walls in meters
    pub wall_thickness_m: f64,
    /// Rim breadth
    pub rim_breadth: f64,
    /// Radius-like control for curved rim lip closure.
    ///
    /// 0.0 keeps a sharp/straight lip transition.
    pub rim_lip_offset_radius: f64,
    /// When true, the curved lip bows on the inner side of the bowl/bell.
    /// When false, the curved lip bows on the outer side.
    pub rim_lip_on_inside: bool,
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

#[derive(Clone, Copy, Debug)]
pub struct NodeControls {
    pub base: BaseControls,
    pub shoulder: ShoulderControls,
    pub fringe: FringeControls,
    pub rim: RimControls,
    pub bow: BowControls,
    pub material: MaterialControls,
}

#[derive(Clone, Copy, Debug)]
pub struct BaseControls {
    pub radius_m: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct ShoulderControls {
    pub radius_m: f64,
    pub sweep_deg: f64,
    pub curves_inward: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct FringeControls {
    pub length_m: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct RimControls {
    pub radius_m: f64,
    pub sweep_deg: f64,
    pub breadth_m: f64,
    pub curves_inward: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct BowControls {
    pub thickness_m: f64,
    pub on_inside: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialControls {
    pub density_kg_per_m3: f64,
    pub wall_thickness_m: f64,
}

#[derive(Clone, Copy)]
struct PenState {
    point: Point2<f64>,
    tangent_angle: f64,
}

#[derive(Clone, Copy)]
struct RimLipArc {
    start_s: f64,
    curve_len: f64,
    max_thickness: f64,
    bow_on_inside: bool,
}

#[derive(Clone, Copy)]
struct RimLipGeometry {
    seg_idx: usize,
    mid_start: Point2<f64>,
    tangent: Point2<f64>,
    rim_len: f64,
    arc: Option<RimLipArc>,
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
const MASS_VOLUME_SAMPLES: usize = 256;

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
    pub fn controls(&self) -> NodeControls {
        NodeControls {
            base: BaseControls {
                radius_m: self.base_radius_m,
            },
            shoulder: ShoulderControls {
                radius_m: self.shoulder_radius_m,
                sweep_deg: self.shoulder_sweep_deg,
                curves_inward: self.shoulder_curves_inward,
            },
            fringe: FringeControls {
                length_m: self.base_length_m,
            },
            rim: RimControls {
                radius_m: self.rim_radius_m,
                sweep_deg: self.rim_sweep_deg,
                breadth_m: self.rim_breadth,
                curves_inward: self.rim_curves_inward,
            },
            bow: BowControls {
                thickness_m: self.rim_lip_offset_radius,
                on_inside: self.rim_lip_on_inside,
            },
            material: MaterialControls {
                density_kg_per_m3: self.material_density_kg_per_m3,
                wall_thickness_m: self.wall_thickness_m,
            },
        }
    }

    pub fn apply_controls(&mut self, controls: NodeControls) {
        self.base_radius_m = controls.base.radius_m;
        self.shoulder_radius_m = controls.shoulder.radius_m;
        self.shoulder_sweep_deg = controls.shoulder.sweep_deg;
        self.shoulder_curves_inward = controls.shoulder.curves_inward;
        self.base_length_m = controls.fringe.length_m;
        self.rim_radius_m = controls.rim.radius_m;
        self.rim_sweep_deg = controls.rim.sweep_deg;
        self.rim_breadth = controls.rim.breadth_m;
        self.rim_curves_inward = controls.rim.curves_inward;
        self.rim_lip_offset_radius = controls.bow.thickness_m;
        self.rim_lip_on_inside = controls.bow.on_inside;
        self.material_density_kg_per_m3 = controls.material.density_kg_per_m3;
        self.wall_thickness_m = controls.material.wall_thickness_m;
    }

    pub fn from_controls(controls: NodeControls) -> Self {
        let mut node = Self {
            material_density_kg_per_m3: 0.0,
            wall_thickness_m: 0.0,
            rim_breadth: 0.0,
            rim_lip_offset_radius: 0.0,
            rim_lip_on_inside: true,
            base_radius_m: 0.0,
            shoulder_radius_m: 0.0,
            shoulder_sweep_deg: 0.0,
            rim_radius_m: 0.0,
            rim_sweep_deg: 0.0,
            shoulder_curves_inward: false,
            rim_curves_inward: false,
            base_length_m: 0.01,
        };
        node.apply_controls(controls);
        node
    }

    fn sweep_abs_deg_to_rad(deg: f64) -> f64 {
        Self::deg_to_rad(deg.abs())
    }

    fn rim_line_segment_index(&self) -> Option<usize> {
        let segments = self.build_outer_segments();
        let last = segments.last().copied()?;
        match last {
            PathSegment::Line { length, .. }
                if length > 0.0 && self.rim_breadth.max(0.0) > EPS_COORD =>
            {
                Some(segments.len() - 1)
            }
            _ => None,
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

    fn add(a: Point2<f64>, b: Point2<f64>) -> Point2<f64> {
        Point2 {
            x: a.x + b.x,
            y: a.y + b.y,
        }
    }

    fn scale(v: Point2<f64>, k: f64) -> Point2<f64> {
        Point2 {
            x: v.x * k,
            y: v.y * k,
        }
    }

    fn build_lip_arc(rim_len: f64, bow_thickness: f64, bow_on_inside: bool) -> Option<RimLipArc> {
        if rim_len <= EPS_COORD || bow_thickness <= EPS_COORD {
            return None;
        }

        // Use the full rim segment as the bow span so the joins are explicit:
        // t=0 joins opposite edge at rim tip, t=1 joins baseline edge at rim start.
        let curve_len = rim_len.max(EPS_COORD);
        if curve_len <= EPS_COORD {
            return None;
        }

        let start_s = 0.0;

        Some(RimLipArc {
            start_s,
            curve_len,
            max_thickness: bow_thickness,
            bow_on_inside,
        })
    }

    fn rim_lip_geometry(&self) -> Option<RimLipGeometry> {
        let seg_idx = self.rim_line_segment_index()?;
        let segments = self.build_outer_segments();
        let segment = segments.get(seg_idx).copied()?;
        let (mid_start, rim_len, tangent) = match segment {
            PathSegment::Line {
                start,
                length,
                tangent_angle,
            } => (
                start,
                length.max(0.0),
                Point2 {
                    x: tangent_angle.cos(),
                    y: tangent_angle.sin(),
                },
            ),
            _ => return None,
        };

        if rim_len <= EPS_COORD {
            return None;
        }

        let bow_thickness = self.rim_lip_offset_radius.max(0.0);
        let bow_on_inside = self.rim_lip_on_inside;
        let arc = if bow_thickness > EPS_COORD {
            Self::build_lip_arc(rim_len, bow_thickness, bow_on_inside)
        } else {
            None
        };

        Some(RimLipGeometry {
            seg_idx,
            mid_start,
            tangent,
            rim_len,
            arc,
        })
    }

    /// Sample the rim outer/inner pair at signed offset `s` from the rim-line start.
    /// Negative `s` values address the preceding segment (when the arc overhangs into it).
    fn sample_rim_outer_inner(
        &self,
        geom: RimLipGeometry,
        s: f64,
        theta: f64,
        clamp_x_to_axis: bool,
    ) -> (Point2<f64>, Point2<f64>) {
        let mid_line = Self::add(geom.mid_start, Self::scale(geom.tangent, s));
        let inward = Self::inward_normal_for_theta(self, theta, geom.tangent, mid_line);
        let half_thickness = 0.5 * self.wall_thickness_m.max(0.0);
        let mut inner_line = Self::add(mid_line, Self::scale(inward, half_thickness));
        let outer_line = Self::add(mid_line, Self::scale(inward, -half_thickness));

        let Some(arc) = geom.arc else {
            if clamp_x_to_axis && inner_line.x < 0.0 {
                inner_line.x = 0.0;
            }
            return (outer_line, inner_line);
        };

        if s < arc.start_s || s > arc.start_s + arc.curve_len {
            if clamp_x_to_axis && inner_line.x < 0.0 {
                inner_line.x = 0.0;
            }
            return (outer_line, inner_line);
        }

        let p = ((s - arc.start_s) / arc.curve_len.max(EPS_COORD)).clamp(0.0, 1.0);
        // t=0 at rim tip (outside-end), t=1 at rim root (inside-end).
        let t = 1.0 - p;
        let smooth_t = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        let peak = (std::f64::consts::PI * t).sin();

        if arc.bow_on_inside {
            let join = Point2 {
                x: outer_line.x + (inner_line.x - outer_line.x) * smooth_t,
                y: outer_line.y + (inner_line.y - outer_line.y) * smooth_t,
            };
            let mut inner = Self::add(join, Self::scale(inward, arc.max_thickness * peak));
            if clamp_x_to_axis && inner.x < 0.0 {
                inner.x = 0.0;
            }
            (outer_line, inner)
        } else {
            let join = Point2 {
                x: inner_line.x + (outer_line.x - inner_line.x) * smooth_t,
                y: inner_line.y + (outer_line.y - inner_line.y) * smooth_t,
            };
            let outward = Self::scale(inward, -1.0);
            let outer = Self::add(join, Self::scale(outward, arc.max_thickness * peak));
            let mut inner = inner_line;
            if clamp_x_to_axis && inner.x < 0.0 {
                inner.x = 0.0;
            }
            (outer, inner)
        }
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

    fn inward_normal_for_theta(
        &self,
        theta: f64,
        tangent: Point2<f64>,
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

        let axis_side = if left_point.x.abs() <= right_point.x.abs() {
            left
        } else {
            right
        };
        let away_from_axis = if axis_side.x == left.x && axis_side.y == left.y {
            right
        } else {
            left
        };

        match self.midpoint_sample_at(theta).convexity {
            geometry::LocalConvexity::Convex => {
                if away_from_axis.x < 0.0 {
                    away_from_axis
                } else {
                    axis_side
                }
            }
            geometry::LocalConvexity::Concave | geometry::LocalConvexity::Flat => axis_side,
        }
    }
}
