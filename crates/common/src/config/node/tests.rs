use mint::Point2;

use super::*;

fn demo_node(shoulder_curves_inward: bool, rim_curves_inward: bool) -> NodePhysics {
    NodePhysics {
        material_density_kg_per_m3: 1_000.0,
        wall_thickness_m: 0.05,
        rim_breadth: 0.1,
        rim_lip_offset_radius: 0.0,
        rim_lip_on_inside: true,
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
            "bell_sharp_rim",
            NodePhysics {
                base_radius_m: 0.24,
                shoulder_radius_m: 0.16,
                rim_radius_m: 0.0,
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
            "gong_rounded_lip",
            NodePhysics {
                wall_thickness_m: 0.04,
                rim_breadth: 0.09,
                rim_lip_offset_radius: 0.11,
                rim_lip_on_inside: true,
                base_radius_m: 0.28,
                shoulder_radius_m: 0.36,
                rim_radius_m: 0.08,
                shoulder_sweep_deg: 12.0,
                rim_sweep_deg: 14.0,
                shoulder_curves_inward: false,
                rim_curves_inward: false,
                base_length_m: 1.14,
                ..demo_node(false, false)
            },
        ),
        (
            "gong_rounded_lip_outside",
            NodePhysics {
                wall_thickness_m: 0.04,
                rim_breadth: 0.09,
                rim_lip_offset_radius: 0.11,
                rim_lip_on_inside: false,
                base_radius_m: 0.28,
                shoulder_radius_m: 0.36,
                rim_radius_m: 0.08,
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

#[test]
fn volume_methods_return_finite_positive_values() {
    let node = demo_node(false, false);

    let inner_volume = node.inner_volume_m3(128);
    let material_volume = node.material_volume_m3(128);

    assert!(inner_volume.is_finite());
    assert!(material_volume.is_finite());
    assert!(inner_volume > 0.0);
    assert!(material_volume > 0.0);
}

#[test]
fn zero_wall_thickness_has_no_material_volume() {
    let node = NodePhysics {
        wall_thickness_m: 0.0,
        ..demo_node(false, false)
    };

    assert!(node.material_volume_m3(128) <= EPS_COORD);
}

#[test]
fn derived_mass_matches_density_times_material_volume() {
    let node = demo_node(true, false);
    let expected_mass =
        node.material_density_kg_per_m3 * node.material_volume_m3(MASS_VOLUME_SAMPLES);

    assert!((node.mass_kg() - expected_mass).abs() <= EPS_COORD);
}

#[test]
fn rim_radius_zero_keeps_sharp_terminal_thickness() {
    let node = NodePhysics {
        rim_lip_offset_radius: 0.0,
        rim_breadth: 0.12,
        ..demo_node(false, true)
    };

    let t_end = node.thickness_at(1.0);
    assert!(t_end > node.wall_thickness_m * 0.9);
}

#[test]
fn positive_rim_radius_rounds_terminal_inner_to_outer_connection() {
    let node = NodePhysics {
        rim_lip_offset_radius: 0.08,
        rim_lip_on_inside: true,
        rim_breadth: 0.12,
        ..demo_node(false, true)
    };

    let t_end = node.thickness_at(1.0);
    let mut t_peak: f64 = 0.0;
    for i in 160..200 {
        let u = i as f64 / 200.0;
        t_peak = t_peak.max(node.thickness_at(u));
    }

    assert!(t_peak > node.wall_thickness_m);
    assert!(t_end <= EPS_COORD);
}

#[test]
fn rim_lip_inside_outside_flip_changes_terminal_shape() {
    let inside = NodePhysics {
        rim_lip_offset_radius: 0.10,
        rim_lip_on_inside: true,
        rim_breadth: 0.14,
        ..demo_node(false, true)
    };
    let outside = NodePhysics {
        rim_lip_offset_radius: 0.10,
        rim_lip_on_inside: false,
        rim_breadth: 0.14,
        ..demo_node(false, true)
    };

    let (_, inside_inner) = inside.sample_outer_inner_at(0.98);
    let (_, outside_inner) = outside.sample_outer_inner_at(0.98);
    assert!((inside_inner.x - outside_inner.x).abs() > 1e-3);
}

#[test]
fn lip_radius_increases_local_rim_thickness_envelope() {
    let base = NodePhysics {
        rim_lip_offset_radius: 0.0,
        rim_breadth: 0.14,
        ..demo_node(false, true)
    };
    let lip = NodePhysics {
        rim_lip_offset_radius: 0.12,
        rim_lip_on_inside: true,
        rim_breadth: 0.14,
        ..demo_node(false, true)
    };

    let mut t_base: f64 = 0.0;
    let mut t_lip: f64 = 0.0;
    for i in 170..200 {
        let u = i as f64 / 200.0;
        t_base = t_base.max(base.thickness_at(u));
        t_lip = t_lip.max(lip.thickness_at(u));
    }

    assert!(t_lip > t_base);
}

#[test]
fn rim_lip_terminal_join_is_closed() {
    let node = NodePhysics {
        rim_lip_offset_radius: 0.10,
        rim_lip_on_inside: false,
        rim_breadth: 0.12,
        ..demo_node(false, true)
    };

    let outline = node.shell_outline(200);
    let first = outline[0];
    let last_before_axis = outline
        .iter()
        .rev()
        .find(|p| p.x.abs() > EPS_COORD || p.y.abs() > EPS_COORD)
        .copied()
        .unwrap_or(first);

    assert!((first.x - last_before_axis.x).abs() <= EPS_COORD);
    assert!((first.y - last_before_axis.y).abs() <= EPS_COORD);

    let (outer_tip, inner_tip) = node.sample_outer_inner_at(1.0);
    let dx = outer_tip.x - inner_tip.x;
    let dy = outer_tip.y - inner_tip.y;
    let tip_thickness = (dx * dx + dy * dy).sqrt();
    assert!(tip_thickness <= EPS_COORD);
}

#[test]
fn shell_outline_last_point_matches_first() {
    let node = NodePhysics {
        rim_lip_offset_radius: 0.10,
        rim_lip_on_inside: true,
        rim_breadth: 0.12,
        ..demo_node(false, true)
    };

    let outline = node.shell_outline(200);
    assert!(!outline.is_empty());

    let first = outline[0];
    let last = outline[outline.len() - 1];
    assert!((first.x - last.x).abs() <= EPS_COORD);
    assert!((first.y - last.y).abs() <= EPS_COORD);
}
