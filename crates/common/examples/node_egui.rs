use common::body::materials::{Material, Medium};
use common::config::{Node, NodeComputedDebug, NodeComputedStructure, NodeModelBuilders};
use common::egui_helpers::{
    draw_xy_line_chart, draw_xy_multi_line_chart_sized, run_native_app,
    show_action_error_messages, show_scrolled_left_panel_inside, CameraControls,
};
use common::Meshable;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke};
use nalgebra::{Point3, Vector3};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

fn main() -> eframe::Result<()> {
    run_native_app("Node inspector", [1480.0, 980.0], |_cc| {
        NodeInspectorApp::default()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExampleShapePreset {
    Bell,
    Bowl,
    Bottle,
}

struct NodeAnalysisInputs {
    builders: NodeModelBuilders,
    bowl_material: Material,
    clapper_material: Material,
    clapper_to_bowl_friction: f64,
    seed: Option<u64>,
    medium: Medium,
    resolution: usize,
    mode_count: usize,
    node_signature: String,
    acoustic_signature: String,
    cached_structure: Option<NodeComputedStructure>,
}

struct PendingAnalysis {
    node_signature: String,
    acoustic_signature: String,
    receiver: Receiver<AnalysisResult>,
}

enum AnalysisResult {
    Ready(ReadyAnalysis),
    Error(String),
}

struct ReadyAnalysis {
    node_signature: String,
    acoustic_signature: String,
    debug: NodeComputedDebug,
}

#[derive(Clone)]
struct DisplayCache {
    bowl_profile: Vec<(f64, f64)>,
    clapper_profile: Vec<(f64, f64)>,
    thickness_face_profile: Vec<(f64, f64)>,
    thickness_back_profile: Vec<(f64, f64)>,
    bowl_preview: MeshPreview,
    clapper_preview: MeshPreview,
    shell_overlay_points: Vec<Point3<f64>>,
}

#[derive(Clone)]
struct MeshPreview {
    points: Vec<Point3<f64>>,
    indices: Vec<[u32; 3]>,
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

struct NodeInspectorApp {
    builders: NodeModelBuilders,
    active_preset: ExampleShapePreset,
    bowl_material: Material,
    clapper_material: Material,
    clapper_to_bowl_friction: f64,
    medium: Medium,
    use_seed: bool,
    seed: u64,
    analysis_resolution: usize,
    mode_count: usize,
    camera: CameraControls,
    wireframe: bool,
    last_error: Option<String>,
    last_debug_snapshot: Option<String>,
    pending_analysis: Option<PendingAnalysis>,
    queued_analysis: Option<NodeAnalysisInputs>,
    ready_analysis: Option<ReadyAnalysis>,
    live_display: Option<DisplayCache>,
    live_display_signature: Option<String>,
    recompute_requested: bool,
}

impl Default for NodeInspectorApp {
    fn default() -> Self {
        Self {
            builders: NodeModelBuilders::default(),
            active_preset: ExampleShapePreset::Bowl,
            bowl_material: Material {
                id: "NODE_EGUI_BOWL".to_string(),
                reference_density_kg_per_m3: 8800.0,
                poisson_ratio: 0.34,
                reference_youngs_modulus_mpa: 110_000.0,
                reference_temperature_c: 20.0,
                linear_thermal_expansion_per_c: 18.0e-6,
                dln_e_dtemp_per_c: -3.0e-4,
            },
            clapper_material: Material {
                id: "NODE_EGUI_CLAPPER".to_string(),
                reference_density_kg_per_m3: 7850.0,
                poisson_ratio: 0.29,
                reference_youngs_modulus_mpa: 200_000.0,
                reference_temperature_c: 20.0,
                linear_thermal_expansion_per_c: 12.0e-6,
                dln_e_dtemp_per_c: -4.0e-4,
            },
            clapper_to_bowl_friction: 0.16,
            medium: Medium::standard_air(),
            use_seed: false,
            seed: 7,
            analysis_resolution: 8,
            mode_count: 8,
            camera: CameraControls::orbit_zoom_pan(0.45, 0.65, 1.45, 0.0, 0.0),
            wireframe: true,
            last_error: None,
            last_debug_snapshot: None,
            pending_analysis: None,
            queued_analysis: None,
            ready_analysis: None,
            live_display: None,
            live_display_signature: None,
            recompute_requested: true,
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

    fn node_signature(&self) -> String {
        serde_json::to_string(&(
            &self.builders,
            &self.bowl_material,
            &self.clapper_material,
            self.clapper_to_bowl_friction,
            self.use_seed,
            self.seed,
            self.analysis_resolution,
            self.mode_count,
        ))
        .unwrap_or_else(|err| format!("signature-error:{err}"))
    }

    fn acoustic_signature(&self) -> String {
        serde_json::to_string(&self.medium).unwrap_or_else(|err| format!("signature-error:{err}"))
    }

    fn analysis_inputs(&self) -> NodeAnalysisInputs {
        let node_signature = self.node_signature();
        let cached_structure = self.ready_analysis.as_ref().and_then(|ready| {
            (ready.node_signature == node_signature).then_some(ready.debug.0.clone())
        });

        NodeAnalysisInputs {
            builders: self.builders.clone(),
            bowl_material: self.bowl_material.clone(),
            clapper_material: self.clapper_material.clone(),
            clapper_to_bowl_friction: self.clapper_to_bowl_friction,
            seed: self.use_seed.then_some(self.seed),
            medium: self.medium.clone(),
            resolution: self.analysis_resolution,
            mode_count: self.mode_count,
            node_signature,
            acoustic_signature: self.acoustic_signature(),
            cached_structure,
        }
    }

    fn display_signature(&self) -> String {
        serde_json::to_string(&(
            &self.builders,
            &self.bowl_material,
            &self.clapper_material,
            self.clapper_to_bowl_friction,
            self.use_seed,
            self.seed,
            self.analysis_resolution,
        ))
        .unwrap_or_else(|err| format!("signature-error:{err}"))
    }

    fn ensure_live_display(&mut self) {
        let signature = self.display_signature();
        if self.live_display_signature.as_deref() == Some(signature.as_str()) {
            return;
        }

        let node = match self.builders.build_node(
            self.bowl_material.clone(),
            self.clapper_material.clone(),
            self.clapper_to_bowl_friction,
            self.use_seed.then_some(self.seed),
        ) {
            Ok(node) => node,
            Err(err) => {
                self.live_display = None;
                self.live_display_signature = None;
                self.last_error = Some(err);
                return;
            }
        };

        self.live_display = Some(build_display_cache(
            &node,
            self.analysis_resolution,
            self.builders.thickness.inner_base_thickness_m,
            self.builders.thickness.inner_lip_thickness_m,
            self.builders.thickness.outer_base_thickness_m,
            self.builders.thickness.outer_lip_thickness_m,
        ));
        self.live_display_signature = Some(signature);
    }

    fn analysis_is_stale(&self) -> bool {
        match &self.ready_analysis {
            Some(ready) => {
                ready.node_signature != self.node_signature()
                    || ready.acoustic_signature != self.acoustic_signature()
            }
            None => true,
        }
    }

    fn ensure_analysis_requested(&mut self) {
        let inputs = self.analysis_inputs();

        let ready_matches = self
            .ready_analysis
            .as_ref()
            .map(|ready| {
                ready.node_signature == inputs.node_signature
                    && ready.acoustic_signature == inputs.acoustic_signature
            })
            .unwrap_or(false);
        if ready_matches {
            return;
        }

        self.ready_analysis = None;

        let pending_matches = self
            .pending_analysis
            .as_ref()
            .map(|pending| {
                pending.node_signature == inputs.node_signature
                    && pending.acoustic_signature == inputs.acoustic_signature
            })
            .unwrap_or(false);
        if pending_matches {
            self.queued_analysis = None;
            return;
        }

        if self.pending_analysis.is_some() {
            // Supersede stale work immediately so the latest caller request does not wait
            // behind an already-running analysis for older inputs.
            self.pending_analysis = Some(spawn_analysis(inputs));
            self.queued_analysis = None;
            self.last_error = None;
            self.last_debug_snapshot = None;
            return;
        }

        self.pending_analysis = Some(spawn_analysis(inputs));
        self.queued_analysis = None;
        self.last_error = None;
        self.last_debug_snapshot = None;
    }

    fn poll_analysis(&mut self) {
        let result = match self.pending_analysis.as_ref() {
            Some(pending) => match pending.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(AnalysisResult::Error(
                    "analysis worker disconnected".to_string(),
                )),
            },
            None => None,
        };

        let Some(result) = result else {
            return;
        };

        self.pending_analysis = None;
        match result {
            AnalysisResult::Ready(ready) => {
                self.last_error = None;
                self.ready_analysis = Some(ready);
            }
            AnalysisResult::Error(message) => {
                self.last_error = Some(message);
            }
        }

        if let Some(next_inputs) = self.queued_analysis.take() {
            self.pending_analysis = Some(spawn_analysis(next_inputs));
            self.last_debug_snapshot = None;
        }
    }

    fn analysis_is_pending(&self) -> bool {
        self.pending_analysis.is_some()
    }

    fn analysis_has_queued_work(&self) -> bool {
        self.queued_analysis.is_some()
    }

    fn print_debug_snapshot_once_for_change(&mut self, debug: &NodeComputedDebug) {
        let (structure, acoustics) = debug;
        let strike_preview = structure
            .strike_modes
            .iter()
            .zip(acoustics.strike_modes.iter())
            .take(3)
            .map(|(s, a)| {
                format!(
                    "S#{:02}@{:.1}Hz[c={:.3},air={:.4},bw={:.1}]",
                    s.mode_index + 1,
                    a.frequency_hz,
                    s.strike_base.coupling,
                    a.damping_in_medium,
                    a.impact_bandwidth_hz
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let jet_preview = format!(
            "J<src={} struct={:.1}Hz lock={:.1}Hz bw={:.1} rim={:.3} air={:.4}>",
            structure.jet_mode.source_mode_index + 1,
            structure.jet_mode.structural_frequency_hz,
            acoustics.jet_mode.acoustic_lock_in.lock_center_hz,
            acoustics.jet_mode.acoustic_lock_in.lock_bandwidth_hz,
            structure.jet_mode.rim_response,
            acoustics.jet_mode.damping_in_medium,
        );
        let slide_preview = structure
            .slide_modes
            .iter()
            .zip(acoustics.slide_modes.iter())
            .take(3)
            .map(|(s, a)| {
                format!(
                    "L#{:02}@{:.1}Hz[c={:.3},slip={:.3},air={:.4},gain={:.3}]",
                    s.mode_index + 1,
                    a.frequency_hz,
                    s.slide_base.coupling,
                    s.slide_base.contact_state.slip_drive,
                    a.damping_in_medium,
                    a.friction_interaction_gain
                )
            })
            .collect::<Vec<_>>()
            .join(" ");

        let snapshot = format!(
            "strike={} slide={} budget={} cond={:.2e} acoustic=[{} | {} | {}]",
            structure.strike_modes.len(),
            structure.slide_modes.len(),
            structure.mode_budget,
            structure.solver_condition_number,
            strike_preview,
            jet_preview,
            slide_preview,
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

        ui.add(
            egui::Slider::new(&mut self.analysis_resolution, 2..=96).text("analysis resolution"),
        );
        ui.add(egui::Slider::new(&mut self.mode_count, 1..=24).text("mode count"));
        if ui.button("Recompute analysis").clicked() {
            self.recompute_requested = true;
        }
        if self.analysis_is_stale() {
            ui.small("Inputs changed. Click Recompute analysis to refresh results.");
        }

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

        egui::CollapsingHeader::new("Random Seed")
            .id_salt("node_seed")
            .default_open(false)
            .show(ui, |ui| {
                ui.checkbox(&mut self.use_seed, "use deterministic seed");
                ui.add_enabled(
                    self.use_seed,
                    egui::DragValue::new(&mut self.seed)
                        .speed(1.0)
                        .range(0_u64..=u64::MAX)
                        .prefix("seed "),
                );
            });

        egui::CollapsingHeader::new("Medium")
            .id_salt("node_medium")
            .default_open(false)
            .show(ui, |ui| {
                if ui.button("Compute Impedance From Density x c").clicked() {
                    self.medium.impedance_m_rayl = Medium::impedance_from_density_and_speed(
                        self.medium.density_kg_per_m3,
                        self.medium.speed_of_sound_m_per_s,
                    );
                }
                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.medium.temperature_c, -40.0..=120.0)
                        .text("temperature (C)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.medium.pressure_pa, 20_000.0..=250_000.0)
                        .logarithmic(true)
                        .text("pressure (Pa)"),
                );

                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.medium.density_kg_per_m3, 0.02..=50.0)
                        .logarithmic(true)
                        .text("density (kg/m^3)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.medium.speed_of_sound_m_per_s, 10.0..=2000.0)
                        .logarithmic(true)
                        .text("c (m/s)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.medium.viscosity_pa_s, 1.0e-7..=1.0e-2)
                        .logarithmic(true)
                        .text("viscosity (Pa*s)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.medium.impedance_m_rayl, 1.0..=20_000.0)
                        .logarithmic(true)
                        .text("impedance (Rayl)"),
                );
            });

        egui::CollapsingHeader::new("Materials")
            .id_salt("node_materials")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Bowl material");
                ui.add(
                    egui::Slider::new(
                        &mut self.bowl_material.reference_density_kg_per_m3,
                        500.0..=20000.0,
                    )
                    .text("bowl density"),
                );
                ui.add(
                    egui::Slider::new(&mut self.bowl_material.poisson_ratio, -0.49..=0.49)
                        .text("bowl poisson"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.bowl_material.reference_youngs_modulus_mpa,
                        1.0e3..=5.0e5,
                    )
                    .logarithmic(true)
                    .text("bowl E (MPa)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.bowl_material.dln_e_dtemp_per_c, -2.0e-3..=2.0e-3)
                        .text("bowl d(ln E)/dT"),
                );
                ui.label("Clapper material");
                ui.add(
                    egui::Slider::new(
                        &mut self.clapper_material.reference_density_kg_per_m3,
                        500.0..=20000.0,
                    )
                    .text("clapper density"),
                );
                ui.add(
                    egui::Slider::new(&mut self.clapper_material.poisson_ratio, -0.49..=0.49)
                        .text("clapper poisson"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.clapper_material.reference_youngs_modulus_mpa,
                        1.0e3..=5.0e5,
                    )
                    .logarithmic(true)
                    .text("clapper E (MPa)"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.clapper_material.dln_e_dtemp_per_c,
                        -2.0e-3..=2.0e-3,
                    )
                    .text("clapper d(ln E)/dT"),
                );
                ui.add(
                    egui::Slider::new(&mut self.clapper_to_bowl_friction, 0.0..=1.5)
                        .text("clapper to bowl friction"),
                );
            });

        egui::CollapsingHeader::new("Preview")
            .id_salt("node_preview")
            .default_open(false)
            .show(ui, |ui| {
                self.camera.show_collapsing(ui, "Camera", "node_camera");
                ui.checkbox(&mut self.wireframe, "wireframe overlay");
                ui.small("Front faces use cool hues, back faces use warm hues.");
                ui.small("Drag in preview to pan");
            });

        ui.separator();
        let action_message = None;
        let error_message = self.last_error.as_ref().map(|err| format!("Error: {err}"));
        show_action_error_messages(ui, &action_message, &error_message);
    }

    fn top_charts(&self, ui: &mut egui::Ui, display: &DisplayCache) {
        ui.columns(3, |cols| {
            draw_xy_line_chart(
                &mut cols[0],
                "Bowl profile",
                &display.bowl_profile,
                "y",
                "r",
            );
            draw_xy_line_chart(
                &mut cols[1],
                "Clapper profile",
                &display.clapper_profile,
                "y",
                "r",
            );
            draw_xy_multi_line_chart_sized(
                &mut cols[2],
                "Thickness profile",
                155.0,
                &[
                    (
                        "face",
                        display.thickness_face_profile.as_slice(),
                        Color32::from_rgb(116, 192, 252),
                    ),
                    (
                        "back",
                        display.thickness_back_profile.as_slice(),
                        Color32::from_rgb(250, 176, 5),
                    ),
                ],
                "idx",
                "m",
            );
        });
    }

    fn draw_3d_preview(
        &mut self,
        ui: &mut egui::Ui,
        display: &DisplayCache,
        debug: Option<&NodeComputedDebug>,
    ) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        if response.dragged() {
            let delta = ui.ctx().input(|input| input.pointer.delta());
            if let Some(pan_x) = &mut self.camera.pan_x {
                *pan_x += delta.x;
            }
            if let Some(pan_y) = &mut self.camera.pan_y {
                *pan_y += delta.y;
            }
        }
        painter.rect_filled(rect, 6.0, Color32::from_rgb(17, 22, 28));

        let zoom = self.camera.zoom.unwrap_or(1.0);
        let pan_x = self.camera.pan_x.unwrap_or(0.0);
        let pan_y = self.camera.pan_y.unwrap_or(0.0);

        let mut tris = Vec::new();
        tris.extend(collect_projected_tris(
            &display.bowl_preview.points,
            &display.bowl_preview.indices,
            self.camera.yaw,
            self.camera.pitch,
            zoom,
            pan_x,
            pan_y,
            rect,
            Color32::from_rgb(91, 151, 219),
            Color32::from_rgb(228, 129, 113),
        ));
        tris.extend(collect_projected_tris(
            &display.clapper_preview.points,
            &display.clapper_preview.indices,
            self.camera.yaw,
            self.camera.pitch,
            zoom,
            pan_x,
            pan_y,
            rect,
            Color32::from_rgb(220, 179, 105),
            Color32::from_rgb(186, 122, 74),
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

        if let Some(debug) = debug {
            let mode_points = collect_mode_overlay_points(
                &display.shell_overlay_points,
                debug,
                self.camera.yaw,
                self.camera.pitch,
                zoom,
                pan_x,
                pan_y,
                rect,
            );
            for point in mode_points {
                painter.circle_filled(point.pos, point.radius_px, point.color);
            }
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
                        "resolution: requested {} / analysis {} / modes {}",
                        structure.requested_resolution,
                        structure.analysis_resolution,
                        structure.strike_modes.len()
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
                        ui.label("Condition number");
                        ui.monospace(format!("{:.3e}", structure.solver_condition_number));
                        ui.end_row();
                        ui.label("Medium temperature (C)");
                        ui.monospace(format!("{:.2}", acoustics.medium.temperature_c));
                        ui.end_row();
                        ui.label("Medium pressure (Pa)");
                        ui.monospace(format!("{:.1}", acoustics.medium.pressure_pa));
                        ui.end_row();
                        ui.label("Medium viscosity");
                        ui.monospace(format!("{:.3e}", acoustics.medium.viscosity_pa_s));
                        ui.end_row();
                    });

                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Mode colors:");
                    for i in 0..structure.strike_modes.len() {
                        let color = mode_color(i);
                        ui.colored_label(color, format!("M{:02}", i + 1));
                    }
                });
                ui.separator();
                ui.label("Strike modes");
                for (i, (strike_mode, strike_acoustics)) in structure
                    .strike_modes
                    .iter()
                    .zip(acoustics.strike_modes.iter())
                    .enumerate()
                {
                    ui.monospace(format!(
                        "S#{:02}  struct={:8.2} Hz  strike={:8.2} Hz\nstrike[c={:.3}, angle={:.3}, damp={:.5}, bw={:.2}]",
                        i + 1,
                        strike_mode.frequency_hz,
                        strike_acoustics.frequency_hz,
                        strike_mode.strike_base.coupling,
                        strike_mode.strike_base.angle_sensitivity,
                        strike_acoustics.damping_in_medium,
                        strike_acoustics.impact_bandwidth_hz,
                    ));
                }

                ui.separator();
                ui.label("Jet mode");
                ui.monospace(format!(
                    "J<src={}>  struct={:8.2} Hz  jet={:8.2} Hz\njet[c={:.3}, rim={:.3}, damp={:.5}, lock={:.2}+/-{:.2}, thr={:.3}, gain={:.3}, tau={:.4}s, St={:.3}, phase={:.3}, rad={:.3e}]",
                    structure.jet_mode.source_mode_index + 1,
                    structure.jet_mode.structural_frequency_hz,
                    acoustics.jet_mode.frequency_hz,
                    structure.jet_mode.jet_base.coupling,
                    structure.jet_mode.rim_response,
                    acoustics.jet_mode.damping_in_medium,
                    acoustics.jet_mode.acoustic_lock_in.lock_center_hz,
                    acoustics.jet_mode.acoustic_lock_in.lock_bandwidth_hz,
                    structure.jet_mode.jet_base.vortex_dynamics.threshold_drive,
                    structure.jet_mode.jet_base.vortex_dynamics.small_signal_gain,
                    structure.jet_mode.jet_base.vortex_dynamics.convective_delay_s,
                    structure.jet_mode.jet_base.vortex_dynamics.strouhal_target,
                    acoustics.jet_mode.acoustic_lock_in.phase_sensitivity,
                    acoustics.jet_mode.radiation_efficiency,
                ));

                ui.separator();
                ui.label("Slide modes");
                for (i, (slide_mode, slide_acoustics)) in structure
                    .slide_modes
                    .iter()
                    .zip(acoustics.slide_modes.iter())
                    .enumerate()
                {
                    ui.monospace(format!(
                        "L#{:02}  struct={:8.2} Hz  slide={:8.2} Hz\nslide[c={:.3}, rough={:.3}, n={:.3}, slip={:.3}, stick={:.3}, int={:.3}, damp={:.5}, bw={:.2}, gain={:.3}, squeal={:.3}]",
                        i + 1,
                        slide_mode.frequency_hz,
                        slide_acoustics.frequency_hz,
                        slide_mode.slide_base.coupling,
                        slide_mode.slide_base.roughness_sensitivity,
                        slide_mode.slide_base.contact_state.normal_load_proxy,
                        slide_mode.slide_base.contact_state.slip_drive,
                        slide_mode.slide_base.contact_state.stick_slip_propensity,
                        slide_mode.slide_base.contact_state.contact_intermittency,
                        slide_acoustics.damping_in_medium,
                        slide_acoustics.slide_bandwidth_hz,
                        slide_acoustics.friction_interaction_gain,
                        slide_acoustics.squeal_tendency,
                    ));
                }
            });
    }
}

impl eframe::App for NodeInspectorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ensure_live_display();
        if self.recompute_requested {
            self.ensure_analysis_requested();
            self.recompute_requested = false;
        }
        self.poll_analysis();

        show_scrolled_left_panel_inside(ui, "node_controls", 300.0, Some(420.0), |ui| {
            self.left_controls(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(display) = self.live_display.clone() {
                let debug_for_overlay = self.ready_analysis.as_ref().and_then(|ready| {
                    (ready.node_signature == self.node_signature()).then_some(ready.debug.clone())
                });

                if let Some(debug) = debug_for_overlay.as_ref() {
                    self.print_debug_snapshot_once_for_change(debug);
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.analysis_is_pending() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Refreshing analysis in the background.");
                        });
                        ui.add_space(6.0);
                    }

                    ui.group(|ui| {
                        self.top_charts(ui, &display);
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 520.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.draw_3d_preview(ui, &display, debug_for_overlay.as_ref());
                            },
                        );
                    });

                    ui.add_space(6.0);
                    if let Some(debug) = debug_for_overlay.as_ref() {
                        ui.group(|ui| {
                            self.bottom_debug(ui, debug);
                        });
                    } else if self.analysis_is_stale() {
                        ui.group(|ui| {
                            ui.label("Analysis is stale. Click Recompute analysis to refresh modal data.");
                        });
                    }
                });
            } else {
                ui.centered_and_justified(|ui| {
                    if self.analysis_is_pending() {
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.label("Computing node analysis...");
                        });
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(255, 120, 120),
                            self.last_error
                                .clone()
                                .unwrap_or_else(|| "unable to build node".to_string()),
                        );
                    }
                });
            }
        });

        if self.analysis_is_pending() || self.analysis_has_queued_work() {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
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

#[derive(Clone, Copy)]
struct DrawPoint {
    pos: Pos2,
    radius_px: f32,
    color: Color32,
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
    front_face_color: Color32,
    back_face_color: Color32,
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

        let light_dir = Vector3::new(0.35, -0.4, -1.0).normalize();
        let lambert = normal.normalize().dot(&light_dir).abs().clamp(0.1, 1.0);
        let shade = 0.25 + lambert * 0.75;
        let base_color = if normal.z < 0.0 {
            front_face_color
        } else {
            back_face_color
        };
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

fn collect_mode_overlay_points(
    points: &[Point3<f64>],
    debug: &NodeComputedDebug,
    yaw: f64,
    pitch: f64,
    zoom: f64,
    pan_x: f32,
    pan_y: f32,
    rect: Rect,
) -> Vec<DrawPoint> {
    let (structure, _) = debug;
    if structure.strike_modes.is_empty() || points.is_empty() {
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

    let stride = (points.len() / 220).max(1);
    let mut overlays = Vec::new();
    for (mode_index, mode) in structure.strike_modes.iter().enumerate() {
        let point_count = points.len().min(mode.vertex_displacement.len());
        if point_count == 0 {
            continue;
        }

        let max_norm = mode
            .vertex_displacement
            .iter()
            .map(|d| d.norm())
            .fold(0.0f64, f64::max)
            .max(1e-9);
        let displacement_scale = (0.15 * structure.bowl_radius_m.max(1e-4)) / max_norm;
        let color = mode_color(mode_index);

        for i in (0..point_count).step_by(stride) {
            let displaced = points[i] + mode.vertex_displacement[i] * displacement_scale;
            let rotated = rotate_point(displaced, yaw, pitch);
            overlays.push(DrawPoint {
                pos: project_to_screen(rotated, rect, scale, pan_x, pan_y),
                radius_px: 1.8,
                color,
            });
        }
    }

    overlays
}

fn mode_color(mode_index: usize) -> Color32 {
    const PALETTE: [Color32; 12] = [
        Color32::from_rgb(255, 99, 132),
        Color32::from_rgb(54, 162, 235),
        Color32::from_rgb(255, 206, 86),
        Color32::from_rgb(75, 192, 192),
        Color32::from_rgb(153, 102, 255),
        Color32::from_rgb(255, 159, 64),
        Color32::from_rgb(46, 204, 113),
        Color32::from_rgb(231, 76, 60),
        Color32::from_rgb(52, 73, 94),
        Color32::from_rgb(26, 188, 156),
        Color32::from_rgb(241, 196, 15),
        Color32::from_rgb(155, 89, 182),
    ];
    PALETTE[mode_index % PALETTE.len()]
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

fn radial_profile_from_points(points: &[Point3<f64>], bins: usize) -> Vec<(f64, f64)> {
    if points.is_empty() || bins < 2 {
        return vec![];
    }

    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in points {
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    if !min_y.is_finite() || !max_y.is_finite() {
        return vec![];
    }
    let span = (max_y - min_y).max(1.0e-12);

    let mut max_radius_by_bin = vec![0.0f64; bins];
    let mut has_point = vec![false; bins];
    for point in points {
        let t = ((point.y - min_y) / span).clamp(0.0, 1.0);
        let index = ((t * (bins - 1) as f64).round() as usize).min(bins - 1);
        let radius = (point.x * point.x + point.z * point.z).sqrt();
        if radius.is_finite() {
            max_radius_by_bin[index] = max_radius_by_bin[index].max(radius);
            has_point[index] = true;
        }
    }

    (0..bins)
        .filter(|&i| has_point[i])
        .map(|i| {
            let y = min_y + (i as f64 / (bins - 1) as f64) * span;
            (y, max_radius_by_bin[i])
        })
        .collect()
}

fn spawn_analysis(inputs: NodeAnalysisInputs) -> PendingAnalysis {
    let node_signature = inputs.node_signature.clone();
    let acoustic_signature = inputs.acoustic_signature.clone();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let result = run_analysis(inputs);
        let _ = sender.send(result);
    });

    PendingAnalysis {
        node_signature,
        acoustic_signature,
        receiver,
    }
}

fn run_analysis(inputs: NodeAnalysisInputs) -> AnalysisResult {
    let node = match inputs.builders.build_node(
        inputs.bowl_material,
        inputs.clapper_material,
        inputs.clapper_to_bowl_friction,
        inputs.seed,
    ) {
        Ok(node) => node,
        Err(err) => return AnalysisResult::Error(err),
    };

    let structure = match inputs.cached_structure {
        Some(cached) => cached,
        None => match node.computed_structure(inputs.resolution, inputs.mode_count) {
            Some(structure) => structure,
            None => return AnalysisResult::Error("computed structure unavailable".to_string()),
        },
    };

    let acoustics = match node.computed_acoustics_from_structure(&structure, &inputs.medium) {
        Some(acoustics) => acoustics,
        None => return AnalysisResult::Error("computed acoustics unavailable".to_string()),
    };
    let debug = (structure, acoustics);

    AnalysisResult::Ready(ReadyAnalysis {
        node_signature: inputs.node_signature,
        acoustic_signature: inputs.acoustic_signature,
        debug,
    })
}

fn build_display_cache(
    node: &Node,
    overlay_resolution: usize,
    inner_base_thickness_m: f64,
    inner_lip_thickness_m: f64,
    outer_base_thickness_m: f64,
    outer_lip_thickness_m: f64,
) -> DisplayCache {
    let bowl_mesh = node.bowl.surface_mesh(80);
    let clapper_mesh = node.clapper.surface_mesh(80);
    let bowl_preview = node.bowl.surface_mesh(28);
    let clapper_preview = node.clapper.surface_mesh(24);
    let shell_resolution = 40;
    let shell_mesh = node.bowl.meshable.surface_mesh_data(shell_resolution);

    let (min_y, max_y) = shell_mesh
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min_v, max_v), p| {
            (min_v.min(p.y), max_v.max(p.y))
        });
    let y_span = (max_y - min_y).abs().max(1.0e-12);
    let bins = 64usize;
    let thickness_face_profile = (0..bins)
        .map(|i| {
            let u = if bins <= 1 {
                0.0
            } else {
                i as f64 / (bins - 1) as f64
            };
            let y = min_y + y_span * u;
            let face =
                inner_base_thickness_m + (inner_lip_thickness_m - inner_base_thickness_m) * u;
            (y, face)
        })
        .collect::<Vec<_>>();
    let thickness_back_profile = (0..bins)
        .map(|i| {
            let u = if bins <= 1 {
                0.0
            } else {
                i as f64 / (bins - 1) as f64
            };
            let y = min_y + y_span * u;
            let back =
                outer_base_thickness_m + (outer_lip_thickness_m - outer_base_thickness_m) * u;
            (y, back)
        })
        .collect::<Vec<_>>();

    DisplayCache {
        bowl_profile: radial_profile_from_points(&bowl_mesh.0, 80),
        clapper_profile: radial_profile_from_points(&clapper_mesh.0, 80),
        thickness_face_profile,
        thickness_back_profile,
        bowl_preview: MeshPreview {
            points: bowl_preview.0,
            indices: bowl_preview.1,
        },
        clapper_preview: MeshPreview {
            points: clapper_preview.0,
            indices: clapper_preview.1,
        },
        // Match modal displacement vertex count by using the same shell path as the solver.
        shell_overlay_points: node
            .bowl
            .meshable
            .inner()
            .base
            .base
            .sample_points(overlay_resolution),
    }
}
