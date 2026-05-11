use std::ops::Add;

use mint::{Point2, Vector3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct NodePhysics {
    /// Mass of the node in kg
    pub mass_kg: f64,
    /// Size of the node in meters
    ///
    /// - `x` width of the node at base
    /// - `z` depth of the node at base
    /// - `y` height of the node
    pub size_m: Vector3<f64>,
    /// Density of the material in kg/m^3
    pub material_density_kg_per_m3: f64,
    /// Thickness of the walls in meters
    pub wall_thickness_m: f64,
    /// Base fillet radius of the node in meters
    pub r1: f64,
    /// Rim curve radius of the node in meters
    pub r2: f64,
    /// R1 Curves inward, _Like the inside of a bowl_?
    pub is_r1_concave: bool,
    /// R2 Curves inward, _Like the inside of a bowl_?
    pub is_r2_concave: bool,
}

impl NodePhysics {
    /// Returns two half-profile outlines as `[outer, inner]`.
    ///
    /// Coordinate system:
    /// - `x`: radius (positive away from center axis)
    /// - `y`: height (increasing downward)
    pub fn outlines(&self, num_samples: usize) -> [Vec<Point2<f64>>; 2] {
        let samples_per_zone = num_samples.max(2);
        let h = self.size_m.y.max(0.0);
        let split_y = h * 0.5;
        let rm_outer = 0.5 * (self.r1 + self.r2);

        let outer_a0 = point(self.r1, 0.0);
        let outer_a1 = point(rm_outer, split_y);
        let outer_b0 = outer_a1;
        let outer_b1 = point(self.r2, h);

        let mut outer = Vec::new();
        append_circular_zone(
            &mut outer,
            outer_a0,
            outer_a1,
            self.is_r1_concave,
            self.r1,
            samples_per_zone,
            false,
        );
        append_circular_zone(
            &mut outer,
            outer_b0,
            outer_b1,
            self.is_r2_concave,
            self.r2,
            samples_per_zone,
            true,
        );

        let inner_r1 = (self.r1 - self.wall_thickness_m).max(0.0);
        let inner_rm = (rm_outer - self.wall_thickness_m).max(0.0);
        let inner_r2 = (self.r2 - self.wall_thickness_m).max(0.0);

        let inner_a0 = point(inner_r1, 0.0);
        let inner_a1 = point(inner_rm, split_y);
        let inner_b0 = inner_a1;
        let inner_b1 = point(inner_r2, h);

        let mut inner = Vec::new();
        append_circular_zone(
            &mut inner,
            inner_a0,
            inner_a1,
            self.is_r1_concave,
            self.r1,
            samples_per_zone,
            false,
        );
        append_circular_zone(
            &mut inner,
            inner_b0,
            inner_b1,
            self.is_r2_concave,
            self.r2,
            samples_per_zone,
            true,
        );

        [outer, inner]
    }
}

fn append_circular_zone(
    out: &mut Vec<Point2<f64>>,
    a: Point2<f64>,
    b: Point2<f64>,
    is_concave: bool,
    curve_radius: f64,
    samples: usize,
    skip_first: bool,
) {
    let points = sample_circular_zone(a, b, is_concave, curve_radius, samples);
    for (i, p) in points.into_iter().enumerate() {
        if skip_first && i == 0 {
            continue;
        }
        out.push(p);
    }
}

fn sample_circular_zone(
    a: Point2<f64>,
    b: Point2<f64>,
    is_concave: bool,
    curve_radius: f64,
    samples: usize,
) -> Vec<Point2<f64>> {
    let sample_count = samples.max(2);
    let mut out = Vec::with_capacity(sample_count);
    let chord = vector(b.x - a.x, b.y - a.y);
    let chord_len = (chord.0 * chord.0 + chord.1 * chord.1).sqrt();

    if chord_len < 1e-9 {
        out.resize(sample_count, a);
        return out;
    }

    // If radius is too small for this chord, clamp to the smallest valid circle.
    let min_radius = chord_len * 0.5 + 1e-9;
    let radius = curve_radius.abs().max(min_radius);
    let half_chord = chord_len * 0.5;
    let center_to_chord = (radius * radius - half_chord * half_chord).sqrt();

    let midpoint = point((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    let mut normal = vector(chord.1 / chord_len, -chord.0 / chord_len);
    // Keep the default arc orientation facing +x as outward.
    if normal.0 < 0.0 {
        normal = vector(-normal.0, -normal.1);
    }

    // Concave means arc bends inward toward the center axis (smaller x).
    let center_sign = if is_concave { 1.0 } else { -1.0 };
    let center = point(
        midpoint.x + center_sign * center_to_chord * normal.0,
        midpoint.y + center_sign * center_to_chord * normal.1,
    );

    let a0 = (a.y - center.y).atan2(a.x - center.x);
    let a1 = (b.y - center.y).atan2(b.x - center.x);
    let sweep = signed_shortest_angle(a1 - a0);

    for i in 0..sample_count {
        let t = i as f64 / (sample_count as f64 - 1.0);
        let angle = a0 + sweep * t;
        out.push(point(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ));
    }

    out
}

fn signed_shortest_angle(mut angle: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    while angle > std::f64::consts::PI {
        angle -= tau;
    }
    while angle < -std::f64::consts::PI {
        angle += tau;
    }
    angle
}

fn point(x: f64, y: f64) -> Point2<f64> {
    Point2 { x, y }
}

fn vector(x: f64, y: f64) -> (f64, f64) {
    (x, y)
}

impl Add for NodePhysics {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let is_r_concave = |a: bool, b: bool| {
            let a = if a { 1 } else { -1 };
            let b = if b { 1 } else { -1 };
            a * b == 1
        };

        Self {
            mass_kg: self.mass_kg + rhs.mass_kg,
            size_m: Vector3 {
                x: self.size_m.x + rhs.size_m.x,
                y: self.size_m.y + rhs.size_m.y,
                z: self.size_m.z + rhs.size_m.z,
            },
            material_density_kg_per_m3: self.material_density_kg_per_m3
                + rhs.material_density_kg_per_m3,
            wall_thickness_m: self.wall_thickness_m + rhs.wall_thickness_m,
            r1: self.r1 + rhs.r1,
            r2: self.r2 + rhs.r2,
            is_r1_concave: is_r_concave(self.is_r1_concave, rhs.is_r1_concave),
            is_r2_concave: is_r_concave(self.is_r2_concave, rhs.is_r2_concave),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translated(points: &[Point2<f64>], dx: f64, dy: f64) -> Vec<Point2<f64>> {
        points
            .iter()
            .map(|p| Point2 {
                x: p.x + dx,
                y: p.y + dy,
            })
            .collect()
    }

    fn demo_node(is_r1_concave: bool, is_r2_concave: bool) -> NodePhysics {
        NodePhysics {
            mass_kg: 1.0,
            size_m: Vector3 {
                x: 0.4,
                y: 1.2,
                z: 0.4,
            },
            material_density_kg_per_m3: 1000.0,
            wall_thickness_m: 0.05,
            r1: 0.32,
            r2: 0.58,
            is_r1_concave,
            is_r2_concave,
        }
    }

    #[test]
    fn outlines_returns_outer_and_inner_with_expected_lengths() {
        let node = demo_node(false, false);
        let [outer, inner] = node.outlines(8);

        assert_eq!(outer.len(), 15);
        assert_eq!(inner.len(), 15);
        assert_eq!(outer.first().unwrap().y, 0.0);
        assert_eq!(outer.last().unwrap().y, node.size_m.y);
        assert_eq!(inner.first().unwrap().y, 0.0);
        assert_eq!(inner.last().unwrap().y, node.size_m.y);
    }

    #[test]
    fn concave_lower_zone_bends_inward_relative_to_convex() {
        let convex = demo_node(false, false);
        let concave = demo_node(true, false);

        let [outer_convex, _] = convex.outlines(9);
        let [outer_concave, _] = concave.outlines(9);

        // Index 4 is the midpoint sample of the first zone when samples_per_zone = 9.
        let first_zone_mid = 4;
        assert!(outer_concave[first_zone_mid].x < outer_convex[first_zone_mid].x);
    }

    #[test]
    fn inner_outline_stays_at_or_inside_outer_outline() {
        let node = demo_node(false, true);
        let [outer, inner] = node.outlines(12);

        assert_eq!(outer.len(), inner.len());
        for (o, i) in outer.iter().zip(inner.iter()) {
            assert!(i.x <= o.x + 1e-9);
        }
    }

    #[test]
    fn outlines_snapshot_shape_variations() {
        let variants = [
            (
                "convex_bell",
                NodePhysics {
                    r1: 0.28,
                    r2: 0.62,
                    is_r1_concave: false,
                    is_r2_concave: false,
                    ..demo_node(false, false)
                },
            ),
            (
                "concave_bowl",
                NodePhysics {
                    r1: 0.52,
                    r2: 0.44,
                    is_r1_concave: true,
                    is_r2_concave: true,
                    ..demo_node(true, true)
                },
            ),
            (
                "mixed_lower_concave",
                NodePhysics {
                    r1: 0.46,
                    r2: 0.58,
                    is_r1_concave: true,
                    is_r2_concave: false,
                    ..demo_node(true, false)
                },
            ),
            (
                "mixed_upper_concave",
                NodePhysics {
                    r1: 0.26,
                    r2: 0.54,
                    is_r1_concave: false,
                    is_r2_concave: true,
                    ..demo_node(false, true)
                },
            ),
            (
                "thick_wall_gong",
                NodePhysics {
                    r1: 0.33,
                    r2: 0.50,
                    wall_thickness_m: 0.12,
                    is_r1_concave: false,
                    is_r2_concave: false,
                    ..demo_node(false, false)
                },
            ),
            (
                "slender_node",
                NodePhysics {
                    r1: 0.22,
                    r2: 0.36,
                    size_m: Vector3 {
                        x: 0.4,
                        y: 1.8,
                        z: 0.4,
                    },
                    wall_thickness_m: 0.04,
                    is_r1_concave: false,
                    is_r2_concave: false,
                    ..demo_node(false, false)
                },
            ),
        ];

        let mut all_paths: Vec<Vec<Point2<f64>>> = Vec::new();
        let mut cursor_x = 0.0;
        let spacing = 1.6;

        for (_, node) in variants {
            let [outer, inner] = node.outlines(20);
            all_paths.push(translated(&outer, cursor_x, 0.0));
            all_paths.push(translated(&inner, cursor_x, 0.0));
            cursor_x += spacing;
        }

        let paths: Vec<&[Point2<f64>]> = all_paths.iter().map(Vec::as_slice).collect();
        crate::assert_paths_points_2d_snapshot!("node_outlines_shape_variations", &paths);
    }
}
