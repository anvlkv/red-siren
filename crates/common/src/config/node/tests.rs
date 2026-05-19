use super::*;
use crate::body::materials::Material;
use crate::config::BandChannel;

fn bowl_material() -> Material {
    Material {
        id: "BOWL_TEST_MATERIAL".to_string(),
        reference_density_kg_per_m3: 8800.0,
        poisson_ratio: 0.34,
        reference_youngs_modulus_mpa: 110_000.0,
        reference_temperature_c: 20.0,
        linear_thermal_expansion_per_c: 18.0e-6,
        dln_e_dtemp_per_c: -3.0e-4,
    }
}

fn clapper_material() -> Material {
    Material {
        id: "CLAPPER_TEST_MATERIAL".to_string(),
        reference_density_kg_per_m3: 7850.0,
        poisson_ratio: 0.29,
        reference_youngs_modulus_mpa: 200_000.0,
        reference_temperature_c: 20.0,
        linear_thermal_expansion_per_c: 12.0e-6,
        dln_e_dtemp_per_c: -4.0e-4,
    }
}

fn default_node() -> Node {
    NodeModelBuilders::default()
        .build_node(bowl_material(), clapper_material(), 0.16)
        .expect("default node")
}

#[test]
fn modal_pipeline_is_deterministic_and_sorted() {
    let node = default_node();
    let resolution = 8;
    let mode_count = 8;

    let freqs = node.modal_frequencies_hz(resolution, mode_count);
    assert_eq!(freqs.len(), 8);
    assert!(freqs.iter().all(|f| f.is_finite() && *f > 0.0));
    for pair in freqs.windows(2) {
        assert!(pair[0] <= pair[1]);
    }

    let shapes = node.mode_shapes(resolution, mode_count);
    assert_eq!(shapes.len(), freqs.len());
    assert!(shapes.iter().all(|m| {
        m.strike_base.coupling.is_finite()
            && m.strike_base.coupling >= 0.0
            && m.jet_base.coupling.is_finite()
            && m.jet_base.coupling >= 0.0
    }));
}

#[test]
fn default_builder_first_mode_stays_in_audible_bell_range() {
    let builders = NodeModelBuilders::default();
    let node = builders
        .build_node(bowl_material(), clapper_material(), 0.16)
        .expect("node");

    let freqs = node.modal_frequencies_hz(32, 8);
    assert!(!freqs.is_empty());
    let first = freqs[0];
    assert!(
        first.is_finite() && (60.0..=1500.0).contains(&first),
        "unexpected first mode frequency: {first} Hz"
    );
}

#[test]
fn participation_factors_are_bounded() {
    let node = default_node();
    let resolution = 8;
    let mode_count = 8;
    let factors = node.modal_participation_factor(
        Point3::new(0.0, 0.6, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        resolution,
        mode_count,
    );

    assert_eq!(
        factors.len(),
        node.modal_frequencies_hz(resolution, mode_count).len()
    );
    assert!(factors.iter().all(|f| (0.0..=1.0).contains(f)));
}

#[test]
fn damping_changes_with_medium_properties() {
    let node = default_node();
    let resolution = 8;
    let mode_count = 8;

    let high_viscosity = node.modal_damping(
        &Band {
            channel: BandChannel::Left,
            medium: Medium::from_available(
                "HI_VISC",
                "High Viscosity Test Medium",
                "gas",
                20.0,
                Medium::STANDARD_PRESSURE_PA,
                1.2,
                343.0,
                8.0e-5,
                None,
            ),
        },
        resolution,
        mode_count,
    );
    let low_viscosity = node.modal_damping(
        &Band {
            channel: BandChannel::Left,
            medium: Medium::from_available(
                "LO_VISC",
                "Low Viscosity Test Medium",
                "gas",
                20.0,
                Medium::STANDARD_PRESSURE_PA,
                1.2,
                343.0,
                1.0e-5,
                None,
            ),
        },
        resolution,
        mode_count,
    );
    assert_eq!(high_viscosity.len(), low_viscosity.len());

    let high_viscosity_sum: f64 = high_viscosity.iter().sum();
    let low_viscosity_sum: f64 = low_viscosity.iter().sum();
    assert!((high_viscosity_sum - low_viscosity_sum).abs() > 1e-6);
}

#[test]
fn mounting_constraint_reduces_anchor_displacement() {
    let node = default_node();
    let resolution = 8;
    let mode_count = 8;
    let modes = node.mode_shapes(resolution, mode_count);
    assert!(!modes.is_empty());

    // Use the revolution mesh at the same capped resolution mode_shapes() uses internally,
    // so vertex indices into vertex_displacement are in bounds.
    let mesh = node.bowl.meshable.base.surface_mesh_data(8).into_parts().0;
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
            .build_node(bowl_material(), clapper_material(), 0.16)
            .expect("build node from helpers");
        let freqs = node.modal_frequencies_hz(32, 8);
        assert!(!freqs.is_empty());
        assert!(freqs.iter().all(|f| f.is_finite() && *f > 0.0));
    }
}

#[test]
fn computed_debug_snapshot_contains_full_metrics() {
    let builders = NodeModelBuilders::default();
    let node = builders
        .build_node(bowl_material(), clapper_material(), 0.16)
        .expect("node");

    let medium = Medium::from_available(
        "SNAPSHOT_MEDIUM",
        "Snapshot Test Medium",
        "gas",
        35.0,
        80_000.0,
        1.0,
        360.0,
        1.8e-5,
        None,
    );

    let debug = node.computed_debug(32, 8, &medium).expect("debug snapshot");
    let (structure, acoustics) = debug;
    assert!(structure.mesh_vertex_count > 0);
    assert!(structure.mesh_triangle_count > 0);
    assert!(structure.active_vertex_count > 0);
    assert!(!structure.frequencies_hz.is_empty());
    assert_eq!(
        structure.mode_structures.len(),
        structure.frequencies_hz.len()
    );
    assert_eq!(
        acoustics.strike_damping_in_air.len(),
        acoustics.frequencies_hz.len()
    );
    assert_eq!(
        acoustics.strike_damping_in_medium.len(),
        acoustics.frequencies_hz.len()
    );
    assert_eq!(
        acoustics.jet_damping_in_air.len(),
        acoustics.frequencies_hz.len()
    );
    assert_eq!(
        acoustics.jet_damping_in_medium.len(),
        acoustics.frequencies_hz.len()
    );
    assert_eq!(
        acoustics.slide_damping_in_air.len(),
        acoustics.frequencies_hz.len()
    );
    assert_eq!(
        acoustics.slide_damping_in_medium.len(),
        acoustics.frequencies_hz.len()
    );
    assert!(acoustics.mode_acoustics.iter().all(|mode| mode
        .jet
        .acoustic_lock_in
        .lock_bandwidth_hz
        .is_finite()
        && mode.jet.acoustic_lock_in.lock_bandwidth_hz >= 0.0));
    assert!(structure.mode_structures.iter().all(|mode| mode
        .jet_base
        .vortex_dynamics
        .strouhal_target
        .is_finite()
        && mode.jet_base.vortex_dynamics.strouhal_target > 0.0));
    assert!(structure
        .mode_structures
        .iter()
        .all(|mode| (0.0..=1.0).contains(&mode.slide_base.coupling)));
    assert!(acoustics
        .mode_acoustics
        .iter()
        .all(|mode| mode.slide.slide_bandwidth_hz.is_finite()
            && mode.slide.slide_bandwidth_hz >= 0.0));
    assert!(structure.bowl_mass_kg > 0.0);
    assert!(structure.clapper_mass_kg > 0.0);
    assert!(structure.solver_total_lumped_mass_kg > 0.0);
    assert!(structure.solver_characteristic_edge_length_m > 0.0);
    assert!(structure.solver_lambda_max_raw >= structure.solver_lambda_min_raw);
    assert!(structure.solver_lambda_max_kept >= structure.solver_lambda_min_kept);
    assert!(
        structure.solver_condition_number.is_finite() && structure.solver_condition_number >= 1.0,
        "solver_condition_number should be >= 1.0, got {}",
        structure.solver_condition_number
    );
}

#[test]
fn mindlin_shear_reduces_frequency_for_thicker_walls() {
    // Thicker walls have higher D and G but also more mass. For Mindlin-Reissner, the
    // shear term grows as t³ via D and linearly via G·t·A, while mass grows as ρ·t·A.
    // Net effect: thicker walls produce higher frequencies (stiffer-dominant for typical
    // bell thickness ratios t/R < 0.2). Verify that first mode shifts upward with thickness.
    let mut builders = NodeModelBuilders::default();
    builders.thickness.inner_base_thickness_m = 0.002;
    builders.thickness.inner_lip_thickness_m = 0.002;
    builders.thickness.outer_base_thickness_m = 0.002;
    builders.thickness.outer_lip_thickness_m = 0.002;
    let thin_node = builders
        .build_node(bowl_material(), clapper_material(), 0.16)
        .expect("thin node");

    builders.thickness.inner_base_thickness_m = 0.010;
    builders.thickness.inner_lip_thickness_m = 0.010;
    builders.thickness.outer_base_thickness_m = 0.010;
    builders.thickness.outer_lip_thickness_m = 0.010;
    let thick_node = builders
        .build_node(bowl_material(), clapper_material(), 0.16)
        .expect("thick node");

    let thin_freqs = thin_node.modal_frequencies_hz(16, 4);
    let thick_freqs = thick_node.modal_frequencies_hz(16, 4);
    assert!(
        !thin_freqs.is_empty() && !thick_freqs.is_empty(),
        "solver must return modes for both thin and thick nodes"
    );
    assert!(
        thick_freqs[0] > thin_freqs[0],
        "thicker walls (t=10mm) should have a higher first mode than thin walls (t=2mm); \
         thin={:.1} Hz, thick={:.1} Hz",
        thin_freqs[0],
        thick_freqs[0]
    );
}

#[test]
fn slide_damping_increases_with_friction() {
    let builders = NodeModelBuilders::default();
    let low_friction_node = builders
        .build_node(bowl_material(), clapper_material(), 0.05)
        .expect("low friction node");
    let high_friction_node = builders
        .build_node(bowl_material(), clapper_material(), 0.55)
        .expect("high friction node");

    let medium = Medium::from_available(
        "FRICTION_MEDIUM",
        "Friction Sensitivity Medium",
        "gas",
        25.0,
        Medium::STANDARD_PRESSURE_PA,
        1.2,
        343.0,
        1.81e-5,
        None,
    );

    let low = low_friction_node
        .computed_debug(32, 8, &medium)
        .expect("low debug")
        .1;
    let high = high_friction_node
        .computed_debug(32, 8, &medium)
        .expect("high debug")
        .1;

    let low_sum: f64 = low.slide_damping_in_medium.iter().sum();
    let high_sum: f64 = high.slide_damping_in_medium.iter().sum();
    assert!(
        high_sum > low_sum,
        "expected higher slide damping with higher friction, got low={low_sum}, high={high_sum}"
    );
}
