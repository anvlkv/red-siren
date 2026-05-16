
use super::*;
use crate::body::materials::Material;
use crate::body::{
    thickness_map_from_axial_samples, RevolutionAxis, RevolutionMesh, Segment, ThickMesh,
};
use crate::config::BandChannel;

fn bowl_material() -> Material {
    Material {
        density_kg_per_m3: 8800.0,
        poisson_ratio: 0.34,
        youngs_modulus_pa: 1.1e11,
    }
}

fn clapper_material() -> Material {
    Material {
        density_kg_per_m3: 7850.0,
        poisson_ratio: 0.29,
        youngs_modulus_pa: 2.0e11,
    }
}

fn make_bowl() -> Body<BowlGeometry> {
    let seg0 = Segment::start_constant(0.2, 0.14).expect("segment");
    let mut seg1 = Segment::start_constant(0.4, 0.16).expect("segment");
    seg1.start = seg0.end;
    let mut seg2 = Segment::start_constant(0.6, 0.19).expect("segment");
    seg2.start = seg1.end;
    let mut seg3 = Segment::start_constant(0.8, 0.17).expect("segment");
    seg3.start = seg2.end;
    let mut seg4 = Segment::start_constant(1.0, 0.1).expect("segment");
    seg4.start = seg3.end;

    let base = RevolutionMesh::new([seg0, seg1, seg2, seg3, seg4], RevolutionAxis::Y)
        .expect("revolution mesh");
    let axial = base.profile_sample_positions(32);
    let map =
        thickness_map_from_axial_samples(&axial, 32, |u| (0.006 + 0.002 * u, 0.005 + 0.001 * u))
            .expect("thickness map");
    Body::new(ThickMesh::new(base, map), bowl_material())
}

fn make_clapper() -> Body<ClapperGeometry> {
    let seg0 = Segment::start_constant(0.1, 0.03).expect("segment");
    let mut seg1 = Segment::start_constant(0.2, 0.045).expect("segment");
    seg1.start = seg0.end;
    let mut seg2 = Segment::start_constant(0.3, 0.02).expect("segment");
    seg2.start = seg1.end;

    let mesh = RevolutionMesh::new([seg0, seg1, seg2], RevolutionAxis::Y).expect("clapper");
    Body::new(mesh, clapper_material())
}

fn make_node() -> Node {
    Node::new(make_bowl(), make_clapper())
}

fn band(viscosity_pa_s: f64) -> Band {
    Band {
        channel: BandChannel::Left,
        medium: Medium {
            density_kg_per_m3: 1.225,
            speed_of_sound_m_per_s: 343.0,
            viscosity_pa_s,
            impedance_m_rayl: 420.0,
        },
    }
}

#[test]
fn modal_pipeline_is_deterministic_and_sorted() {
    let node = make_node();
    let resolution = 32;

    let freqs = node.modal_frequencies_hz(resolution);
    assert_eq!(freqs.len(), 8);
    assert!(freqs.iter().all(|f| f.is_finite() && *f > 0.0));
    for pair in freqs.windows(2) {
        assert!(pair[0] <= pair[1]);
    }

    let shapes = node.mode_shapes(resolution);
    assert_eq!(shapes.len(), freqs.len());
    assert!(shapes.iter().all(|m| m.paths.strike.damping_in_air >= 0.0));
    assert!(shapes.iter().all(|m| m.paths.jet.damping_in_air >= 0.0));
}

#[test]
fn participation_factors_are_bounded() {
    let node = make_node();
    let resolution = 32;
    let factors = node.modal_participation_factor(
        Point3::new(0.0, 0.6, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        resolution,
    );

    assert_eq!(factors.len(), node.modal_frequencies_hz(resolution).len());
    assert!(factors.iter().all(|f| (0.0..=1.0).contains(f)));
}

#[test]
fn damping_increases_with_viscosity() {
    let node = make_node();
    let resolution = 32;

    let low = node.modal_damping(&band(1.8e-5), resolution);
    let high = node.modal_damping(&band(8.0e-4), resolution);
    assert_eq!(low.len(), high.len());

    let low_sum: f64 = low.iter().sum();
    let high_sum: f64 = high.iter().sum();
    assert!(high_sum > low_sum);
}

#[test]
fn mounting_constraint_reduces_anchor_displacement() {
    let node = make_node();
    let resolution = 32;
    let modes = node.mode_shapes(resolution);
    assert!(!modes.is_empty());

    // Use the same capped resolution that mode_shapes() uses internally, so
    // vertex indices into vertex_displacement are in bounds.
    let mesh = node
        .bowl
        .meshable
        .surface_mesh_data(MODAL_EIGEN_MAX_RESOLUTION)
        .into_parts()
        .0;
    assert!(!mesh.is_empty());

    let anchor = Point3::new(MOUNT_PROFILE_R_M, MOUNT_PROFILE_Y_M, 0.0);
    let anchor_idx = nearest_vertex_index(&mesh, anchor);
    let far_idx = mesh
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            let da = (*a - anchor).norm_squared();
            let db = (*b - anchor).norm_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(idx, _)| idx)
        .expect("far vertex");

    let first_mode = &modes[0];
    let anchor_amp = first_mode.vertex_displacement[anchor_idx].norm();
    let far_amp = first_mode.vertex_displacement[far_idx].norm();
    assert!(far_amp >= anchor_amp);
}

#[test]
fn model_builders_construct_all_shape_families() {
    let mut builders = NodeModelBuilders::default();
    let presets = [
        (0.9, [0.07, 0.10, 0.18, 0.22, 0.24, 0.27], 0.28),
        (0.75, [0.06, 0.08, 0.16, 0.18, 0.21, 0.22], 0.0),
        (0.95, [0.09, 0.16, 0.17, 0.13, 0.06, 0.05], -0.2),
    ];

    for (height, radii, bias) in presets {
        builders.shape.height_m = height;
        builders.shape.radius_0_m = radii[0];
        builders.shape.radius_1_m = radii[1];
        builders.shape.radius_2_m = radii[2];
        builders.shape.radius_3_m = radii[3];
        builders.shape.radius_4_m = radii[4];
        builders.shape.radius_5_m = radii[5];
        builders.shape.profile_bias = bias;
        let node = builders
            .build_node(bowl_material(), clapper_material())
            .expect("build node from helpers");
        let freqs = node.modal_frequencies_hz(24);
        assert!(!freqs.is_empty());
        assert!(freqs.iter().all(|f| f.is_finite() && *f > 0.0));
    }
}

#[test]
fn computed_debug_snapshot_contains_full_metrics() {
    let builders = NodeModelBuilders::default();
    let node = builders
        .build_node(bowl_material(), clapper_material())
        .expect("node");

    let medium = Medium {
        density_kg_per_m3: 1.4,
        speed_of_sound_m_per_s: 330.0,
        viscosity_pa_s: 4.2e-4,
        impedance_m_rayl: 500.0,
    };

    let debug = node.computed_debug(24, &medium).expect("debug snapshot");
    assert!(debug.mesh_vertex_count > 0);
    assert!(debug.mesh_triangle_count > 0);
    assert!(debug.active_vertex_count > 0);
    assert!(!debug.frequencies_hz.is_empty());
    assert_eq!(debug.mode_shapes.len(), debug.frequencies_hz.len());
    assert_eq!(
        debug.strike_damping_in_air.len(),
        debug.frequencies_hz.len()
    );
    assert_eq!(
        debug.strike_damping_in_medium.len(),
        debug.frequencies_hz.len()
    );
    assert_eq!(debug.jet_damping_in_air.len(), debug.frequencies_hz.len());
    assert_eq!(
        debug.jet_damping_in_medium.len(),
        debug.frequencies_hz.len()
    );
    assert!(debug
        .mode_shapes
        .iter()
        .all(|mode| mode.paths.jet.lock_bandwidth_hz.is_finite()
            && mode.paths.jet.lock_bandwidth_hz >= 0.0));
    assert!(debug
        .mode_shapes
        .iter()
        .all(|mode| mode.paths.jet.strouhal_target.is_finite()
            && mode.paths.jet.strouhal_target > 0.0));
    assert!(debug.bowl_mass_kg > 0.0);
    assert!(debug.clapper_mass_kg > 0.0);
}
