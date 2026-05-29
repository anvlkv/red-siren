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
        .build_node(bowl_material(), clapper_material(), 0.16, None)
        .expect("default node")
}

#[test]
fn modal_pipeline_is_deterministic_and_sorted() {
    let node = default_node();
    let resolution = 3;
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
            && m.strike_base.angle_sensitivity.is_finite()
    }));
}

#[test]
fn path_acoustics_expose_distinct_effective_frequencies() {
    let node = default_node();
    let medium = Medium::standard_air();

    let debug = node
        .computed_debug(8, 4, &medium)
        .expect("computed debug snapshot");
    let (structure, acoustics) = debug;

    assert!(!structure.strike_modes.is_empty());
    assert_eq!(structure.strike_modes.len(), acoustics.strike_modes.len());
    assert_eq!(structure.slide_modes.len(), acoustics.slide_modes.len());

    let first_structural = structure.strike_modes[0].frequency_hz;
    let first_strike = &acoustics.strike_modes[0];
    let first_slide = &acoustics.slide_modes[0];
    let jet_mode = &acoustics.jet_mode;

    assert_eq!(first_strike.frequency_hz, first_structural);
    assert!(first_slide.frequency_hz.is_finite() && first_slide.frequency_hz > 0.0);
    assert!(jet_mode.frequency_hz.is_finite() && jet_mode.frequency_hz > 0.0);
    assert!(
        (jet_mode.frequency_hz - structure.jet_mode.structural_frequency_hz).abs()
            > structure.jet_mode.structural_frequency_hz * 0.05,
        "expected jet frequency to differ from structural mode; structural={} jet={}",
        structure.jet_mode.structural_frequency_hz,
        jet_mode.frequency_hz
    );
    assert_eq!(
        jet_mode.frequency_hz,
        jet_mode.acoustic_lock_in.lock_center_hz
    );
    assert!((first_slide.frequency_hz - first_structural).abs() <= first_structural * 0.12);
}

#[test]
fn default_builder_first_mode_stays_in_audible_bell_range() {
    let builders = NodeModelBuilders::default();
    let node = builders
        .build_node(bowl_material(), clapper_material(), 0.16, None)
        .expect("node");

    let freqs = node.modal_frequencies_hz(3, 8);
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
    let resolution = 3;
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
    let resolution = 3;
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
    let resolution = 3;
    let mode_count = 8;
    let modes = node.mode_shapes(resolution, mode_count);
    assert!(!modes.is_empty());

    let mesh = node
        .bowl
        .meshable
        .inner()
        .base
        .base
        .surface_mesh_data(resolution)
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
            .build_node(bowl_material(), clapper_material(), 0.16, None)
            .expect("build node from helpers");
        let freqs = node.modal_frequencies_hz(3, 8);
        assert!(!freqs.is_empty());
        assert!(freqs.iter().all(|f| f.is_finite() && *f > 0.0));
    }
}

#[test]
#[ignore = "slow high-resolution integration"]
fn computed_debug_snapshot_contains_full_metrics() {
    let builders = NodeModelBuilders::default();
    let node = builders
        .build_node(bowl_material(), clapper_material(), 0.16, None)
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

    let debug = node.computed_debug(3, 8, &medium).expect("debug snapshot");
    let (structure, acoustics) = debug;
    assert!(structure.mesh_vertex_count > 0);
    assert!(structure.mesh_triangle_count > 0);
    assert!(structure.active_vertex_count > 0);
    assert!(!structure.strike_modes.is_empty());
    assert_eq!(structure.strike_modes.len(), acoustics.strike_modes.len());
    assert_eq!(structure.slide_modes.len(), acoustics.slide_modes.len());
    assert!(
        acoustics.jet_mode.acoustic_lock_in.lock_bandwidth_hz.is_finite()
            && acoustics.jet_mode.acoustic_lock_in.lock_bandwidth_hz >= 0.0
    );
    assert!(
        structure.jet_mode.jet_base.vortex_dynamics.strouhal_target.is_finite()
            && structure.jet_mode.jet_base.vortex_dynamics.strouhal_target > 0.0
            && (0.0..=1.0).contains(&structure.jet_mode.rim_response)
    );
    assert!(structure.strike_modes.iter().all(|strike| {
        (0.0..=1.0).contains(&strike.strike_base.coupling) && strike.frequency_hz.is_finite()
    }));
    assert!(structure.slide_modes.iter().all(|slide| {
        (0.0..=1.0).contains(&slide.slide_base.coupling)
            && (0.0..=1.0).contains(&slide.slide_base.contact_state.normal_load_proxy)
            && (0.0..=1.0).contains(&slide.slide_base.contact_state.slip_drive)
            && (0.0..=1.0).contains(&slide.slide_base.contact_state.stick_slip_propensity)
            && (0.0..=1.0).contains(&slide.slide_base.contact_state.contact_intermittency)
    }));
    assert!(acoustics.slide_modes.iter().all(|mode| {
        mode.slide_bandwidth_hz.is_finite()
            && mode.slide_bandwidth_hz >= 0.0
            && (0.0..=1.0).contains(&mode.friction_interaction_gain)
    }));
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
    let mut builders = NodeModelBuilders::default();
    builders.thickness.inner_base_thickness_m = 0.002;
    builders.thickness.inner_lip_thickness_m = 0.002;
    builders.thickness.outer_base_thickness_m = 0.002;
    builders.thickness.outer_lip_thickness_m = 0.002;

    let mut light_clapper = clapper_material();
    light_clapper.reference_density_kg_per_m3 = 1.0;

    let thin_node = builders
        .build_node(bowl_material(), light_clapper.clone(), 0.16, Some(7))
        .expect("thin node");

    builders.thickness.inner_base_thickness_m = 0.030;
    builders.thickness.inner_lip_thickness_m = 0.030;
    builders.thickness.outer_base_thickness_m = 0.030;
    builders.thickness.outer_lip_thickness_m = 0.030;
    let thick_node = builders
        .build_node(bowl_material(), light_clapper, 0.16, Some(7))
        .expect("thick node");

    let thin_freqs = thin_node.modal_frequencies_hz(3, 4);
    let thick_freqs = thick_node.modal_frequencies_hz(3, 4);
    assert!(
        !thin_freqs.is_empty() && !thick_freqs.is_empty(),
        "solver must return modes for both thin and thick nodes"
    );
    assert!(
        thick_freqs[0] < thin_freqs[0],
        "expected lower first mode for very thick shell; thin={:.3}Hz thick={:.3}Hz",
        thin_freqs[0],
        thick_freqs[0]
    );
}

#[test]
fn slide_damping_increases_with_friction() {
    let builders = NodeModelBuilders::default();
    let low_friction_node = builders
        .build_node(bowl_material(), clapper_material(), 0.05, None)
        .expect("low friction node");
    let high_friction_node = builders
        .build_node(bowl_material(), clapper_material(), 0.55, None)
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
        .computed_debug(3, 8, &medium)
        .expect("low debug");
    let high = high_friction_node
        .computed_debug(3, 8, &medium)
        .expect("high debug");

    let low_structure = &low.0;
    let low_acoustics = &low.1;
    let high_structure = &high.0;
    let high_acoustics = &high.1;

    let mean_metric = |modes: &[SlideModeStructure], extractor: fn(&SlideContactState) -> f64| -> f64 {
        let values = modes
            .iter()
            .map(|mode| extractor(&mode.slide_base.contact_state))
            .collect::<Vec<_>>();
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    };

    let low_sum: f64 = low_acoustics.slide_modes.iter().map(|m| m.damping_in_air).sum();
    let high_sum: f64 = high_acoustics.slide_modes.iter().map(|m| m.damping_in_air).sum();
    assert!(
        high_sum > low_sum,
        "expected higher slide damping with higher friction, got low={low_sum}, high={high_sum}"
    );

    let low_slip_drive = mean_metric(&low_structure.slide_modes, |s| s.slip_drive);
    let high_slip_drive = mean_metric(&high_structure.slide_modes, |s| s.slip_drive);
    assert!(
        high_slip_drive > low_slip_drive,
        "expected higher slip drive with higher friction, got low={low_slip_drive}, high={high_slip_drive}"
    );

    let low_gain: f64 = low_acoustics
        .slide_modes
        .iter()
        .map(|mode| mode.friction_interaction_gain)
        .sum();
    let high_gain: f64 = high_acoustics
        .slide_modes
        .iter()
        .map(|mode| mode.friction_interaction_gain)
        .sum();
    assert!(
        (high_gain - low_gain).abs() > 1e-6,
        "expected friction interaction gain to change with friction, got low={low_gain}, high={high_gain}"
    );
    assert!(
        low_acoustics
            .slide_modes
            .iter()
            .all(|mode| (0.0..=1.0).contains(&mode.friction_interaction_gain))
    );
    assert!(
        high_acoustics
            .slide_modes
            .iter()
            .all(|mode| (0.0..=1.0).contains(&mode.friction_interaction_gain))
    );
}
