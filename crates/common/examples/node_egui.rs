use common::body::materials::{Material, Medium};
use common::config::{Node, NodeComputedDebug, NodeModelBuilders};
use common::egui_helpers::{draw_xy_line_chart, run_native_app};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke};
use nalgebra::{Point3, Vector3};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExampleShapePreset {
    Bell,
    Bowl,
    Bottle,
}

fn main() -> eframe::Result<()> {
    run_native_app("Node inspector", [1480.0, 980.0], |_cc| {
        NodeInspectorApp::default()
    })
}

struct NodeInspectorApp {
    builders: NodeModelBuilders,
    active_preset: ExampleShapePreset,
    bowl_material: Material,
    clapper_material: Material,
    medium: Medium,
    resolution: usize,
    mode_count: usize,
    camera_yaw: f64,
    camera_pitch: f64,
    zoom: f64,
    pan_x: f32,
    pan_y: f32,
    wireframe: bool,
    last_error: Option<String>,
    last_debug_snapshot: Option<String>,
}

#[derive(Clone, Copy)]
struct ShapeControlValues {
    height: f64,
    radius_0: f64,
    radius_1: f64,
    radius_2: f64,
    radius_3: f64,
    radius_4: f64,
    radius_5: f64,
    profile_bias: f64,
    segment_bias_0: f64,
    segment_bias_1: f64,
    segment_bias_2: f64,
    segment_bias_3: f64,
    segment_bias_4: f64,
    sampling_density: f64,
}

impl Default for NodeInspectorApp {
    fn default() -> Self {
        Self {
            builders: NodeModelBuilders::default(),
            active_preset: ExampleShapePreset::Bowl,
            bowl_material: Material {
                density_kg_per_m3: 8800.0,
                poisson_ratio: 0.34,
                youngs_modulus_pa: 1.1e11,
            },
            clapper_material: Material {
                density_kg_per_m3: 7850.0,
                poisson_ratio: 0.29,
                youngs_modulus_pa: 2.0e11,
            },
            medium: Medium {
                density_kg_per_m3: 1.225,
                speed_of_sound_m_per_s: 343.0,
                viscosity_pa_s: 1.8e-5,
                impedance_m_rayl: 420.0,
            },
            resolution: 40,
            mode_count: 8,
            camera_yaw: 0.65,
            camera_pitch: 0.45,
            zoom: 1.45,
            pan_x: 0.0,
            pan_y: 0.0,
            wireframe: true,
            last_error: None,
            last_debug_snapshot: None,
        }
    }
}

impl NodeInspectorApp {
    fn propose_shape_values(&mut self, kind: ExampleShapePreset) {
        self.active_preset = kind;
        let shape = &mut self.builders.shape;
        match kind {
            ExampleShapePreset::Bell => {
                shape.height_m = 0.9;
                shape.radius_0_m = 0.07;
                shape.radius_1_m = 0.10;
                shape.radius_2_m = 0.18;
                shape.radius_3_m = 0.22;
                shape.radius_4_m = 0.24;
                shape.radius_5_m = 0.27;
                shape.profile_bias = 0.28;
            }
            ExampleShapePreset::Bowl => {
                shape.height_m = 0.75;
                shape.radius_0_m = 0.06;
                shape.radius_1_m = 0.08;
                shape.radius_2_m = 0.16;
                shape.radius_3_m = 0.18;
                shape.radius_4_m = 0.21;
                shape.radius_5_m = 0.22;
                shape.profile_bias = 0.0;
            }
            ExampleShapePreset::Bottle => {
                shape.height_m = 0.95;
                shape.radius_0_m = 0.09;
                shape.radius_1_m = 0.16;
                shape.radius_2_m = 0.17;
                shape.radius_3_m = 0.13;
                shape.radius_4_m = 0.06;
                shape.radius_5_m = 0.05;
                shape.profile_bias = -0.2;
            }
        }
        shape.segment_bias_0 = 0.0;
        shape.segment_bias_1 = 0.0;
        shape.segment_bias_2 = 0.0;
        shape.segment_bias_3 = 0.0;
        shape.segment_bias_4 = 0.0;
        shape.sampling_density = 24.0;
    }

    fn shape_control_values(&self) -> ShapeControlValues {
        ShapeControlValues {
            height: self.builders.shape.height_m,
            radius_0: self.builders.shape.radius_0_m,
            radius_1: self.builders.shape.radius_1_m,
            radius_2: self.builders.shape.radius_2_m,
            radius_3: self.builders.shape.radius_3_m,
            radius_4: self.builders.shape.radius_4_m,
            radius_5: self.builders.shape.radius_5_m,
            profile_bias: self.builders.shape.profile_bias,
            segment_bias_0: self.builders.shape.segment_bias_0,
            segment_bias_1: self.builders.shape.segment_bias_1,
            segment_bias_2: self.builders.shape.segment_bias_2,
            segment_bias_3: self.builders.shape.segment_bias_3,
            segment_bias_4: self.builders.shape.segment_bias_4,
            sampling_density: self.builders.shape.sampling_density,
        }
    }

    fn apply_shape_control_values(&mut self, controls: ShapeControlValues) {
        self.builders.shape.height_m = controls.height;
        self.builders.shape.radius_0_m = controls.radius_0;
        self.builders.shape.radius_1_m = controls.radius_1;
        self.builders.shape.radius_2_m = controls.radius_2;
        self.builders.shape.radius_3_m = controls.radius_3;
        self.builders.shape.radius_4_m = controls.radius_4;
        self.builders.shape.radius_5_m = controls.radius_5;
        self.builders.shape.profile_bias = controls.profile_bias;
        self.builders.shape.segment_bias_0 = controls.segment_bias_0;
        self.builders.shape.segment_bias_1 = controls.segment_bias_1;
        self.builders.shape.segment_bias_2 = controls.segment_bias_2;
        self.builders.shape.segment_bias_3 = controls.segment_bias_3;
        self.builders.shape.segment_bias_4 = controls.segment_bias_4;
        self.builders.shape.sampling_density = controls.sampling_density;
    }

    fn build_node_and_debug(&mut self) -> Option<(Node, NodeComputedDebug)> {
        match self
            .builders
            .build_node(self.bowl_material, self.clapper_material)
        {
            Ok(node) => match node.computed_debug(self.resolution, self.mode_count, &self.medium) {
                Some(debug) => {
                    self.last_error = None;
                    Some((node, debug))
                }
                None => {
                    self.last_error = Some("computed debug snapshot unavailable".to_string());
                    None
                }
            },
            Err(err) => {
                self.last_error = Some(err);
                None
            }
        }
    }

    fn print_debug_snapshot_once_for_change(&mut self, debug: &NodeComputedDebug) {
        let (structure, acoustics) = debug;
        let acoustic_preview = debug
            .1
            .frequencies_hz
            .iter()
            .enumerate()
            .take(4)
            .map(|(i, freq)| {
                let mode_structure = &structure.mode_structures[i];
                let mode_acoustics = &acoustics.mode_acoustics[i];
                format!(
                    "#{:02} {:.1}Hz strike[c={:.3},air={:.4},bw={:.1}] jet[c={:.3},air={:.4},lock={:.1}±{:.1},thr={:.3},gain={:.3},tau={:.4},St={:.3}]",
                    i + 1,
                    freq,
                    mode_structure.strike_base.coupling,
                    mode_acoustics.strike.damping_in_air,
                    mode_acoustics.strike.impact_bandwidth_hz,
                    mode_structure.jet_base.coupling,
                    mode_acoustics.jet.damping_in_air,
                    mode_acoustics.jet.acoustic_lock_in.lock_center_hz,
                    mode_acoustics.jet.acoustic_lock_in.lock_bandwidth_hz,
                    mode_structure.jet_base.vortex_dynamics.threshold_drive,
                    mode_structure.jet_base.vortex_dynamics.small_signal_gain,
                    mode_structure.jet_base.vortex_dynamics.convective_delay_s,
                    mode_structure.jet_base.vortex_dynamics.strouhal_target,
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");

        let snapshot = format!(
            "modes={} budget={} acoustic=[{}]",
            structure.frequencies_hz.len(),
            structure.mode_budget,
            acoustic_preview,
        );
        if self.last_debug_snapshot.as_deref() == Some(snapshot.as_str()) {
            return;
        }

        self.last_debug_snapshot = Some(snapshot.clone());
        println!("[node_egui] {snapshot}");
    }

    fn left_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Node Helpers");
        ui.label("Generic profile builder with example presets");

        ui.add(egui::Slider::new(&mut self.resolution, 12..=96).text("analysis resolution"));
        ui.add(egui::Slider::new(&mut self.mode_count, 1..=24).text("mode count"));

        egui::CollapsingHeader::new("Shape Profile")
            .id_salt("node_shape_profile")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Propose Bell").clicked() {
                        self.propose_shape_values(ExampleShapePreset::Bell);
                    }
                    if ui.button("Propose Bowl").clicked() {
                        self.propose_shape_values(ExampleShapePreset::Bowl);
                    }
                    if ui.button("Propose Bottle").clicked() {
                        self.propose_shape_values(ExampleShapePreset::Bottle);
                    }
                });

                let mut controls = self.shape_control_values();
                ui.add(egui::Slider::new(&mut controls.height, 0.3..=1.8).text("height"));
                ui.add(egui::Slider::new(&mut controls.radius_0, 0.005..=0.45).text("radius 0"));
                ui.add(egui::Slider::new(&mut controls.radius_1, 0.005..=0.55).text("radius 1"));
                ui.add(egui::Slider::new(&mut controls.radius_2, 0.005..=0.65).text("radius 2"));
                ui.add(egui::Slider::new(&mut controls.radius_3, 0.005..=0.65).text("radius 3"));
                ui.add(egui::Slider::new(&mut controls.radius_4, 0.005..=0.65).text("radius 4"));
                ui.add(egui::Slider::new(&mut controls.radius_5, 0.005..=0.65).text("radius 5"));
                ui.add(
                    egui::Slider::new(&mut controls.profile_bias, -1.0..=1.0).text("profile bias"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.segment_bias_0, -1.0..=1.0)
                        .text("segment bias 0"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.segment_bias_1, -1.0..=1.0)
                        .text("segment bias 1"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.segment_bias_2, -1.0..=1.0)
                        .text("segment bias 2"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.segment_bias_3, -1.0..=1.0)
                        .text("segment bias 3"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.segment_bias_4, -1.0..=1.0)
                        .text("segment bias 4"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.sampling_density, 2.0..=64.0)
                        .text("profile density"),
                );
                self.apply_shape_control_values(controls);
            });

        egui::CollapsingHeader::new("Clapper")
            .id_salt("node_clapper")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.builders.clapper.length_m, 0.15..=0.7)
                        .text("length"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.clapper.head_radius_m, 0.01..=0.08)
                        .text("head r"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.clapper.neck_radius_m, 0.005..=0.06)
                        .text("neck r"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.clapper.tip_radius_m, 0.003..=0.04)
                        .text("tip r"),
                );
            });

        egui::CollapsingHeader::new("Thickness")
            .id_salt("node_thickness")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(
                        &mut self.builders.thickness.inner_base_thickness_m,
                        0.001..=0.02,
                    )
                    .text("inner base"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.builders.thickness.inner_lip_thickness_m,
                        0.001..=0.02,
                    )
                    .text("inner lip"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.builders.thickness.outer_base_thickness_m,
                        0.001..=0.02,
                    )
                    .text("outer base"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.builders.thickness.outer_lip_thickness_m,
                        0.001..=0.02,
                    )
                    .text("outer lip"),
                );
            });

        egui::CollapsingHeader::new("Medium")
            .id_salt("node_medium")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.medium.density_kg_per_m3, 0.2..=5.0)
                        .text("density"),
                );
                ui.add(
                    egui::Slider::new(&mut self.medium.speed_of_sound_m_per_s, 120.0..=900.0)
                        .text("c (m/s)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.medium.viscosity_pa_s, 1.0e-6..=5.0e-3)
                        .logarithmic(true)
                        .text("viscosity"),
                );
                ui.add(
                    egui::Slider::new(&mut self.medium.impedance_m_rayl, 10.0..=5000.0)
                        .logarithmic(true)
                        .text("impedance"),
                );
            });

        egui::CollapsingHeader::new("Materials")
            .id_salt("node_materials")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Bowl material");
                ui.add(
                    egui::Slider::new(&mut self.bowl_material.density_kg_per_m3, 500.0..=20000.0)
                        .text("bowl density"),
                );
                ui.add(
                    egui::Slider::new(&mut self.bowl_material.poisson_ratio, -0.49..=0.49)
                        .text("bowl poisson"),
                );
                ui.add(
                    egui::Slider::new(&mut self.bowl_material.youngs_modulus_pa, 1.0e9..=5.0e11)
                        .logarithmic(true)
                        .text("bowl E"),
                );
                ui.label("Clapper material");
                ui.add(
                    egui::Slider::new(
                        &mut self.clapper_material.density_kg_per_m3,
                        500.0..=20000.0,
                    )
                    .text("clapper density"),
                );
                ui.add(
                    egui::Slider::new(&mut self.clapper_material.poisson_ratio, -0.49..=0.49)
                        .text("clapper poisson"),
                );
                ui.add(
                    egui::Slider::new(&mut self.clapper_material.youngs_modulus_pa, 1.0e9..=5.0e11)
                        .logarithmic(true)
                        .text("clapper E"),
                );
            });

        egui::CollapsingHeader::new("Preview")
            .id_salt("node_preview")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut self.camera_yaw, -3.14..=3.14).text("yaw"));
                ui.add(egui::Slider::new(&mut self.camera_pitch, -1.35..=1.35).text("pitch"));
                ui.add(egui::Slider::new(&mut self.zoom, 0.35..=3.5).text("zoom"));
                ui.add(egui::Slider::new(&mut self.pan_x, -480.0..=480.0).text("pan x"));
                ui.add(egui::Slider::new(&mut self.pan_y, -360.0..=360.0).text("pan y"));
                ui.checkbox(&mut self.wireframe, "wireframe overlay");
                ui.small("Drag in preview to pan");
            });

        if let Some(err) = &self.last_error {
            ui.separator();
            ui.colored_label(Color32::from_rgb(255, 120, 120), format!("Error: {err}"));
        }
    }

    fn top_charts(&self, ui: &mut egui::Ui, node: &Node) {
        ui.columns(3, |cols| {
            let bowl_pts = node.bowl.meshable.base.sampled_profile_points(80);
            draw_xy_line_chart(&mut cols[0], "Bowl profile", &bowl_pts, "y", "r");

            let clapper_pts = node.clapper.meshable.sampled_profile_points(80);
            draw_xy_line_chart(&mut cols[1], "Clapper profile", &clapper_pts, "y", "r");

            let radial = node.bowl.meshable.base.profile_sample_positions(40);
            let thickness_pts = radial
                .iter()
                .enumerate()
                .map(|(i, u)| {
                    let (face, back) = node.bowl.meshable.thickness_map.sample(i);
                    (*u, face + back)
                })
                .collect::<Vec<_>>();
            draw_xy_line_chart(&mut cols[2], "Thickness profile", &thickness_pts, "u", "m");
        });
    }

    fn draw_3d_preview(&mut self, ui: &mut egui::Ui, node: &Node) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        if response.dragged() {
            let delta = ui.ctx().input(|input| input.pointer.delta());
            self.pan_x += delta.x;
            self.pan_y += delta.y;
        }
        painter.rect_filled(rect, 6.0, Color32::from_rgb(17, 22, 28));

        let bowl_mesh = node.bowl.surface_mesh(28);
        let clapper_mesh = node.clapper.surface_mesh(24);

        let mut tris = Vec::new();
        tris.extend(collect_projected_tris(
            &bowl_mesh.0,
            &bowl_mesh.1,
            self.camera_yaw,
            self.camera_pitch,
            self.zoom,
            self.pan_x,
            self.pan_y,
            rect,
            Color32::from_rgb(91, 151, 219),
        ));
        tris.extend(collect_projected_tris(
            &clapper_mesh.0,
            &clapper_mesh.1,
            self.camera_yaw,
            self.camera_pitch,
            self.zoom,
            self.pan_x,
            self.pan_y,
            rect,
            Color32::from_rgb(220, 179, 105),
        ));

        tris.sort_by(|a, b| {
            a.depth
                .partial_cmp(&b.depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for tri in tris {
            painter.add(Shape::convex_polygon(
                vec![tri.p0, tri.p1, tri.p2],
                tri.fill,
                if self.wireframe {
                    Stroke::new(0.6_f32, Color32::from_rgba_premultiplied(20, 20, 20, 150))
                } else {
                    Stroke::NONE
                },
            ));
        }
    }

    fn bottom_debug(&self, ui: &mut egui::Ui, debug: &NodeComputedDebug) {
        let (structure, acoustics) = debug;
        egui::ScrollArea::vertical()
            .id_salt("node_inspector_scroll")
            .max_height(220.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "resolution: req {} / analysis {} / modes {}",
                        structure.requested_resolution,
                        structure.analysis_resolution,
                        structure.frequencies_hz.len()
                    ));
                    ui.separator();
                    ui.label(format!(
                        "mesh: {} vertices, {} triangles",
                        structure.mesh_vertex_count, structure.mesh_triangle_count
                    ));
                    ui.separator();
                    ui.label(format!(
                        "active/constrained: {}/{}",
                        structure.active_vertex_count, structure.constrained_vertex_count
                    ));
                });

                egui::Grid::new("node_debug_grid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Bowl mass (kg)");
                        ui.monospace(format!("{:.5}", structure.bowl_mass_kg));
                        ui.end_row();
                        ui.label("Clapper mass (kg)");
                        ui.monospace(format!("{:.5}", structure.clapper_mass_kg));
                        ui.end_row();
                        ui.label("Clapper ratio");
                        ui.monospace(format!("{:.5}", structure.clapper_mass_ratio));
                        ui.end_row();
                        ui.label("Bowl radius (m)");
                        ui.monospace(format!("{:.5}", structure.bowl_radius_m));
                        ui.end_row();
                        ui.label("Bowl thickness (m)");
                        ui.monospace(format!("{:.6}", structure.bowl_thickness_m));
                        ui.end_row();
                        ui.label("Surface area (m^2)");
                        ui.monospace(format!("{:.5}", structure.bowl_surface_area_m2));
                        ui.end_row();
                        ui.label("Volume (m^3)");
                        ui.monospace(format!("{:.6}", structure.bowl_volume_m3));
                        ui.end_row();
                        ui.label("Solver lumped mass (kg)");
                        ui.monospace(format!("{:.5}", structure.solver_total_lumped_mass_kg));
                        ui.end_row();
                        ui.label("Solver edge length (m)");
                        ui.monospace(format!(
                            "{:.6}",
                            structure.solver_characteristic_edge_length_m
                        ));
                        ui.end_row();
                        ui.label("Flexural rigidity");
                        ui.monospace(format!("{:.3e}", structure.bowl_flexural_rigidity));
                        ui.end_row();
                        ui.label("Lambda raw min/max");
                        ui.monospace(format!(
                            "{:.3e} / {:.3e}",
                            structure.solver_lambda_min_raw, structure.solver_lambda_max_raw
                        ));
                        ui.end_row();
                        ui.label("Lambda kept min/max");
                        ui.monospace(format!(
                            "{:.3e} / {:.3e}",
                            structure.solver_lambda_min_kept, structure.solver_lambda_max_kept
                        ));
                        ui.end_row();
                        ui.label("Dropped eigvals");
                        ui.monospace(format!(
                            "nf={} np={} rigid={}",
                            structure.solver_dropped_non_finite,
                            structure.solver_dropped_non_positive,
                            structure.solver_dropped_near_rigid,
                        ));
                        ui.end_row();
                        ui.label("Medium viscosity");
                        ui.monospace(format!("{:.3e}", acoustics.medium.viscosity_pa_s));
                        ui.end_row();
                    });

                ui.separator();
                ui.label("Modes");
                for i in 0..structure.mode_structures.len() {
                    let mode_structure = &structure.mode_structures[i];
                    let mode_acoustics = &acoustics.mode_acoustics[i];
                    let freq = mode_structure.frequency_hz;
                    let strike_damp_air = acoustics
                        .strike_damping_in_air
                        .get(i)
                        .copied()
                        .unwrap_or(0.0);
                    let strike_damp_medium = acoustics
                        .strike_damping_in_medium
                        .get(i)
                        .copied()
                        .unwrap_or(0.0);
                    let jet_damp_air = acoustics.jet_damping_in_air.get(i).copied().unwrap_or(0.0);
                    let jet_damp_medium =
                        acoustics.jet_damping_in_medium.get(i).copied().unwrap_or(0.0);
                    ui.monospace(format!(r#"#{:02}  {:8.2} Hz  
strike[c={:.3}, damp(a/m)={:.5}/{:.5}, angle={:.3}, bw={:.2}]  
jet[c={:.3}, damp(a/m)={:.5}/{:.5}, lock={:.2}+/-{:.2}, thr={:.3}, gain={:.3}, tau={:.4}s, St={:.3}, phase={:.3}, rad={:.3e}]"#,
                        i + 1,
                        freq,
                        mode_structure.strike_base.coupling,
                        strike_damp_air,
                        strike_damp_medium,
                        mode_structure.strike_base.angle_sensitivity,
                        mode_acoustics.strike.impact_bandwidth_hz,
                        mode_structure.jet_base.coupling,
                        jet_damp_air,
                        jet_damp_medium,
                        mode_acoustics.jet.acoustic_lock_in.lock_center_hz,
                        mode_acoustics.jet.acoustic_lock_in.lock_bandwidth_hz,
                        mode_structure.jet_base.vortex_dynamics.threshold_drive,
                        mode_structure.jet_base.vortex_dynamics.small_signal_gain,
                        mode_structure.jet_base.vortex_dynamics.convective_delay_s,
                        mode_structure.jet_base.vortex_dynamics.strouhal_target,
                        mode_acoustics.jet.acoustic_lock_in.phase_sensitivity,
                        mode_acoustics.jet.radiation_efficiency,
                    ));
                }
            });
    }
}

impl eframe::App for NodeInspectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.build_node_and_debug();

        egui::SidePanel::left("node_controls")
            .min_width(300.0)
            .max_width(420.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.left_controls(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some((node, debug)) = state {
                self.print_debug_snapshot_once_for_change(&debug);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.group(|ui| {
                        self.top_charts(ui, &node);
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 520.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.draw_3d_preview(ui, &node);
                            },
                        );
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        self.bottom_debug(ui, &debug);
                    });
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(
                        Color32::from_rgb(255, 120, 120),
                        self.last_error
                            .clone()
                            .unwrap_or_else(|| "unable to build node".to_string()),
                    );
                });
            }
        });

        ctx.request_repaint();
    }
}

#[derive(Clone)]
struct DrawTri {
    p0: Pos2,
    p1: Pos2,
    p2: Pos2,
    depth: f64,
    fill: Color32,
}

fn collect_projected_tris(
    points: &[Point3<f64>],
    indices: &[[u32; 3]],
    yaw: f64,
    pitch: f64,
    zoom: f64,
    pan_x: f32,
    pan_y: f32,
    rect: Rect,
    base_color: Color32,
) -> Vec<DrawTri> {
    if points.is_empty() || indices.is_empty() {
        return vec![];
    }

    let mut transformed = Vec::with_capacity(points.len());
    for p in points {
        transformed.push(rotate_point(*p, yaw, pitch));
    }

    let mut max_abs = 0.0f64;
    for p in &transformed {
        max_abs = max_abs.max(p.x.abs()).max(p.y.abs()).max(p.z.abs());
    }
    let scale = (rect.width().min(rect.height()) as f64) * 0.42 * zoom / max_abs.max(1e-6);

    let mut tris = Vec::with_capacity(indices.len());
    for [ia, ib, ic] in indices {
        let (ia, ib, ic) = (*ia as usize, *ib as usize, *ic as usize);
        if ia >= transformed.len() || ib >= transformed.len() || ic >= transformed.len() {
            continue;
        }

        let a = transformed[ia];
        let b = transformed[ib];
        let c = transformed[ic];
        let normal = (b - a).cross(&(c - a));
        if normal.z >= 0.0 {
            continue;
        }

        let light_dir = Vector3::new(0.35, -0.4, -1.0).normalize();
        let lambert = normal.normalize().dot(&light_dir).abs().clamp(0.1, 1.0);
        let shade = 0.25 + lambert * 0.75;
        let fill = scale_color(base_color, shade as f32);

        let p0 = project_to_screen(a, rect, scale, pan_x, pan_y);
        let p1 = project_to_screen(b, rect, scale, pan_x, pan_y);
        let p2 = project_to_screen(c, rect, scale, pan_x, pan_y);
        let depth = (a.z + b.z + c.z) / 3.0;

        tris.push(DrawTri {
            p0,
            p1,
            p2,
            depth,
            fill,
        });
    }

    tris
}

fn rotate_point(p: Point3<f64>, yaw: f64, pitch: f64) -> Point3<f64> {
    let cy = yaw.cos();
    let sy = yaw.sin();
    let cp = pitch.cos();
    let sp = pitch.sin();

    let x = p.x * cy + p.z * sy;
    let z0 = -p.x * sy + p.z * cy;
    let y = p.y * cp - z0 * sp;
    let z = p.y * sp + z0 * cp;

    Point3::new(x, y, z)
}

fn project_to_screen(p: Point3<f64>, rect: Rect, scale: f64, pan_x: f32, pan_y: f32) -> Pos2 {
    let perspective = 1.0 / (1.0 + (p.z + 1.8).max(0.05) * 0.25);
    Pos2::new(
        rect.center().x + pan_x + (p.x * scale * perspective) as f32,
        rect.center().y + pan_y - (p.y * scale * perspective) as f32,
    )
}

fn scale_color(c: Color32, factor: f32) -> Color32 {
    let r = (c.r() as f32 * factor).clamp(0.0, 255.0) as u8;
    let g = (c.g() as f32 * factor).clamp(0.0, 255.0) as u8;
    let b = (c.b() as f32 * factor).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(r, g, b)
}
