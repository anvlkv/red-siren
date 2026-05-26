use common::body::materials::{Material, Medium};
use common::config::{
    JetModeStructure, JetStructuralBase, JetVortexDynamics, Node, NodeComputedDebug,
    NodeComputedStructure, NodeModelBuilders, PathMode, SlideContactState, SlideModeStructure,
    SlideStructuralBase, StrikeModeStructure, StrikeStructuralBase,
};
use common::egui_helpers::{draw_xy_line_chart, draw_xy_multi_line_chart_sized, run_native_app};
use common::Meshable;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke};
use nalgebra::{Point3, Vector3};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

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
    debug_signature: String,
}

struct PendingAnalysis {
    node_signature: String,
    debug_signature: String,
    receiver: Receiver<AnalysisResult>,
}

enum AnalysisResult {
    Ready(ReadyAnalysis),
    Error(String),
}

struct ReadyAnalysis {
    node_signature: String,
    debug_signature: String,
    debug: NodeComputedDebug,
    display: DisplayCache,
}

struct DisplayCache {
    bowl_profile: Vec<(f64, f64)>,
    clapper_profile: Vec<(f64, f64)>,
    thickness_face_profile: Vec<(f64, f64)>,
    thickness_back_profile: Vec<(f64, f64)>,
    bowl_preview: MeshPreview,
    clapper_preview: MeshPreview,
    shell_overlay_points: Vec<Point3<f64>>,
}

struct MeshPreview {
    points: Vec<Point3<f64>>,
    indices: Vec<[u32; 3]>,
}

fn main() -> eframe::Result<()> {
    run_native_app("Node inspector", [1480.0, 980.0], |_cc| {
        NodeInspectorApp::default()
    })
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
    camera_yaw: f64,
    camera_pitch: f64,
    zoom: f64,
    pan_x: f32,
    pan_y: f32,
    wireframe: bool,
    last_error: Option<String>,
    last_debug_snapshot: Option<String>,
    pending_analysis: Option<PendingAnalysis>,
    queued_analysis: Option<NodeAnalysisInputs>,
    ready_analysis: Option<ReadyAnalysis>,
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
            camera_yaw: 0.65,
            camera_pitch: 0.45,
            zoom: 1.45,
            pan_x: 0.0,
            pan_y: 0.0,
            wireframe: true,
            last_error: None,
            last_debug_snapshot: None,
            pending_analysis: None,
            queued_analysis: None,
            ready_analysis: None,
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
        ))
        .unwrap_or_else(|err| format!("signature-error:{err}"))
    }

    fn debug_signature(&self) -> String {
        serde_json::to_string(&(self.analysis_resolution, self.mode_count, &self.medium))
            .unwrap_or_else(|err| format!("signature-error:{err}"))
    }

    fn analysis_inputs(&self) -> NodeAnalysisInputs {
        NodeAnalysisInputs {
            builders: self.builders.clone(),
            bowl_material: self.bowl_material.clone(),
            clapper_material: self.clapper_material.clone(),
            clapper_to_bowl_friction: self.clapper_to_bowl_friction,
            seed: self.use_seed.then_some(self.seed),
            medium: self.medium.clone(),
            resolution: self.analysis_resolution,
            mode_count: self.mode_count,
            node_signature: self.node_signature(),
            debug_signature: self.debug_signature(),
        }
    }

    fn ensure_analysis_requested(&mut self) {
        let inputs = self.analysis_inputs();

        let ready_matches = self
            .ready_analysis
            .as_ref()
            .map(|ready| {
                ready.node_signature == inputs.node_signature
                    && ready.debug_signature == inputs.debug_signature
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
                    && pending.debug_signature == inputs.debug_signature
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
        let mode_view = build_path_mode_view(structure);
        let acoustic_preview = debug
            .1
            .frequencies_hz
            .iter()
            .enumerate()
            .take(4)
            .map(|(i, freq)| {
                let strike_base = mode_view
                    .strike
                    .get(i)
                    .and_then(|m| *m)
                    .map(|m| m.strike_base)
                    .unwrap_or(StrikeStructuralBase {
                        coupling: 0.0,
                        angle_sensitivity: 0.0,
                    });
                let jet = mode_view
                    .jet
                    .get(i)
                    .and_then(|m| *m)
                    .copied()
                    .unwrap_or(JetModeStructure {
                        mode_index: i,
                        rim_response: 0.0,
                        jet_base: JetStructuralBase {
                            coupling: 0.0,
                            vortex_dynamics: JetVortexDynamics {
                                strouhal_target: 0.0,
                                convective_delay_s: 0.0,
                                threshold_drive: 0.0,
                                small_signal_gain: 0.0,
                            },
                        },
                    });
                let slide_base = mode_view
                    .slide
                    .get(i)
                    .and_then(|m| *m)
                    .map(|m| m.slide_base)
                    .unwrap_or(SlideStructuralBase {
                        coupling: 0.0,
                        roughness_sensitivity: 0.0,
                        contact_state: SlideContactState {
                            normal_load_proxy: 0.0,
                            slip_drive: 0.0,
                            stick_slip_propensity: 0.0,
                            contact_intermittency: 0.0,
                        },
                    });
                let mode_acoustics = &acoustics.mode_acoustics[i];
                format!(
                    "#{:02} {:.1}Hz strike[c={:.3},air={:.4},bw={:.1}] jet[c={:.3},rim={:.3},air={:.4},lock={:.1}±{:.1},thr={:.3},gain={:.3},tau={:.4},St={:.3}] slide[c={:.3},n={:.3},slip={:.3},stick={:.3},int={:.3},air={:.4},bw={:.1},gain={:.3},squeal={:.3}]",
                    i + 1,
                    freq,
                    strike_base.coupling,
                    mode_acoustics.strike.damping_in_air,
                    mode_acoustics.strike.impact_bandwidth_hz,
                    jet.jet_base.coupling,
                    jet.rim_response,
                    mode_acoustics.jet.damping_in_air,
                    mode_acoustics.jet.acoustic_lock_in.lock_center_hz,
                    mode_acoustics.jet.acoustic_lock_in.lock_bandwidth_hz,
                    jet.jet_base.vortex_dynamics.threshold_drive,
                    jet.jet_base.vortex_dynamics.small_signal_gain,
                    jet.jet_base.vortex_dynamics.convective_delay_s,
                    jet.jet_base.vortex_dynamics.strouhal_target,
                    slide_base.coupling,
                    slide_base.contact_state.normal_load_proxy,
                    slide_base.contact_state.slip_drive,
                    slide_base.contact_state.stick_slip_propensity,
                    slide_base.contact_state.contact_intermittency,
                    mode_acoustics.slide.damping_in_air,
                    mode_acoustics.slide.slide_bandwidth_hz,
                    mode_acoustics.slide.friction_interaction_gain,
                    mode_acoustics.slide.squeal_tendency,
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");

        let snapshot = format!(
            "modes={} budget={} cond={:.2e} acoustic=[{}]",
            structure.frequencies_hz.len(),
            structure.mode_budget,
            structure.solver_condition_number,
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

        ui.add(
            egui::Slider::new(&mut self.analysis_resolution, 2..=96).text("analysis resolution"),
        );
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
        debug: &NodeComputedDebug,
    ) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        if response.dragged() {
            let delta = ui.ctx().input(|input| input.pointer.delta());
            self.pan_x += delta.x;
            self.pan_y += delta.y;
        }
        painter.rect_filled(rect, 6.0, Color32::from_rgb(17, 22, 28));

        let mut tris = Vec::new();
        tris.extend(collect_projected_tris(
            &display.bowl_preview.points,
            &display.bowl_preview.indices,
            self.camera_yaw,
            self.camera_pitch,
            self.zoom,
            self.pan_x,
            self.pan_y,
            rect,
            Color32::from_rgb(91, 151, 219),
        ));
        tris.extend(collect_projected_tris(
            &display.clapper_preview.points,
            &display.clapper_preview.indices,
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

        let mode_points = collect_mode_overlay_points(
            &display.shell_overlay_points,
            debug,
            self.camera_yaw,
            self.camera_pitch,
            self.zoom,
            self.pan_x,
            self.pan_y,
            rect,
        );
        for point in mode_points {
            painter.circle_filled(point.pos, point.radius_px, point.color);
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
                    for i in 0..structure.frequencies_hz.len() {
                        let color = mode_color(i);
                        ui.colored_label(color, format!("M{:02}", i + 1));
                    }
                });
                ui.separator();
                ui.label("Modes");
                let mode_view = build_path_mode_view(structure);
                for i in 0..structure.frequencies_hz.len() {
                    let strike_base = mode_view
                        .strike
                        .get(i)
                        .and_then(|m| *m)
                        .map(|m| m.strike_base)
                        .unwrap_or(StrikeStructuralBase {
                            coupling: 0.0,
                            angle_sensitivity: 0.0,
                        });
                    let jet = mode_view
                        .jet
                        .get(i)
                        .and_then(|m| *m)
                        .copied()
                        .unwrap_or(JetModeStructure {
                            mode_index: i,
                            rim_response: 0.0,
                            jet_base: JetStructuralBase {
                                coupling: 0.0,
                                vortex_dynamics: JetVortexDynamics {
                                    strouhal_target: 0.0,
                                    convective_delay_s: 0.0,
                                    threshold_drive: 0.0,
                                    small_signal_gain: 0.0,
                                },
                            },
                        });
                    let slide_base = mode_view
                        .slide
                        .get(i)
                        .and_then(|m| *m)
                        .map(|m| m.slide_base)
                        .unwrap_or(SlideStructuralBase {
                            coupling: 0.0,
                            roughness_sensitivity: 0.0,
                            contact_state: SlideContactState {
                                normal_load_proxy: 0.0,
                                slip_drive: 0.0,
                                stick_slip_propensity: 0.0,
                                contact_intermittency: 0.0,
                            },
                        });
                    let mode_acoustics = &acoustics.mode_acoustics[i];
                    let structural_frequency_hz = structure.frequencies_hz[i];
                    let strike_frequency_hz = mode_acoustics.strike.frequency_hz;
                    let jet_frequency_hz = mode_acoustics.jet.frequency_hz;
                    let slide_frequency_hz = mode_acoustics.slide.frequency_hz;
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
                    let slide_damp_air =
                        acoustics.slide_damping_in_air.get(i).copied().unwrap_or(0.0);
                    let slide_damp_medium =
                        acoustics.slide_damping_in_medium.get(i).copied().unwrap_or(0.0);
                    ui.monospace(format!(r#"#{:02}  struct={:8.2} Hz  strike={:8.2} Hz  jet={:8.2} Hz  slide={:8.2} Hz  
strike[c={:.3}, damp(a/m)={:.5}/{:.5}, angle={:.3}, bw={:.2}]  
jet[c={:.3}, rim={:.3}, damp(a/m)={:.5}/{:.5}, lock={:.2}+/-{:.2}, thr={:.3}, gain={:.3}, tau={:.4}s, St={:.3}, phase={:.3}, rad={:.3e}]  
slide[c={:.3}, damp(a/m)={:.5}/{:.5}, rough={:.3}, n={:.3}, slip={:.3}, stick={:.3}, int={:.3}, bw={:.2}, gain={:.3}, squeal={:.3}]"#,
                        i + 1,
                        structural_frequency_hz,
                        strike_frequency_hz,
                        jet_frequency_hz,
                        slide_frequency_hz,
                        strike_base.coupling,
                        strike_damp_air,
                        strike_damp_medium,
                        strike_base.angle_sensitivity,
                        mode_acoustics.strike.impact_bandwidth_hz,
                        jet.jet_base.coupling,
                        jet.rim_response,
                        jet_damp_air,
                        jet_damp_medium,
                        mode_acoustics.jet.acoustic_lock_in.lock_center_hz,
                        mode_acoustics.jet.acoustic_lock_in.lock_bandwidth_hz,
                        jet.jet_base.vortex_dynamics.threshold_drive,
                        jet.jet_base.vortex_dynamics.small_signal_gain,
                        jet.jet_base.vortex_dynamics.convective_delay_s,
                        jet.jet_base.vortex_dynamics.strouhal_target,
                        mode_acoustics.jet.acoustic_lock_in.phase_sensitivity,
                        mode_acoustics.jet.radiation_efficiency,
                        slide_base.coupling,
                        slide_damp_air,
                        slide_damp_medium,
                        slide_base.roughness_sensitivity,
                        slide_base.contact_state.normal_load_proxy,
                        slide_base.contact_state.slip_drive,
                        slide_base.contact_state.stick_slip_propensity,
                        slide_base.contact_state.contact_intermittency,
                        mode_acoustics.slide.slide_bandwidth_hz,
                        mode_acoustics.slide.friction_interaction_gain,
                        mode_acoustics.slide.squeal_tendency,
                    ));
                }
            });
    }
}

impl eframe::App for NodeInspectorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ensure_analysis_requested();
        self.poll_analysis();

        egui::Panel::left("node_controls")
            .min_size(300.0)
            .max_size(420.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.left_controls(ui);
                });
            });

        self.ensure_analysis_requested();
        self.poll_analysis();

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(ready) = self.ready_analysis.take() {
                self.print_debug_snapshot_once_for_change(&ready.debug);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.analysis_is_pending() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Refreshing analysis in the background.");
                        });
                        ui.add_space(6.0);
                    }

                    ui.group(|ui| {
                        self.top_charts(ui, &ready.display);
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 520.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.draw_3d_preview(ui, &ready.display, &ready.debug);
                            },
                        );
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        self.bottom_debug(ui, &ready.debug);
                    });
                });
                self.ready_analysis = Some(ready);
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

struct PathModeView<'a> {
    strike: Vec<Option<&'a StrikeModeStructure>>,
    jet: Vec<Option<&'a JetModeStructure>>,
    slide: Vec<Option<&'a SlideModeStructure>>,
}

fn build_path_mode_view(structure: &NodeComputedStructure) -> PathModeView<'_> {
    let mode_len = structure.frequencies_hz.len();
    let mut view = PathModeView {
        strike: vec![None; mode_len],
        jet: vec![None; mode_len],
        slide: vec![None; mode_len],
    };

    for path_mode in &structure.path_modes {
        match path_mode {
            PathMode::Strike(mode) if mode.mode_index < mode_len => {
                view.strike[mode.mode_index] = Some(mode);
            }
            PathMode::Jet(mode) if mode.mode_index < mode_len => {
                view.jet[mode.mode_index] = Some(mode);
            }
            PathMode::Slide(mode) if mode.mode_index < mode_len => {
                view.slide[mode.mode_index] = Some(mode);
            }
            _ => {}
        }
    }

    view
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
    let mode_view = build_path_mode_view(structure);
    if mode_view.strike.iter().all(|m| m.is_none()) || points.is_empty() {
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
    for (mode_index, mode_opt) in mode_view.strike.iter().enumerate() {
        let Some(mode) = mode_opt else {
            continue;
        };
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
    let debug_signature = inputs.debug_signature.clone();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let result = run_analysis(inputs);
        let _ = sender.send(result);
    });

    PendingAnalysis {
        node_signature,
        debug_signature,
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

    let Some(debug) = node.computed_debug(inputs.resolution, inputs.mode_count, &inputs.medium)
    else {
        return AnalysisResult::Error("computed debug snapshot unavailable".to_string());
    };

    AnalysisResult::Ready(ReadyAnalysis {
        node_signature: inputs.node_signature,
        debug_signature: inputs.debug_signature,
        display: build_display_cache(
            &node,
            &debug,
            inputs.builders.thickness.inner_base_thickness_m,
            inputs.builders.thickness.inner_lip_thickness_m,
            inputs.builders.thickness.outer_base_thickness_m,
            inputs.builders.thickness.outer_lip_thickness_m,
        ),
        debug,
    })
}

fn build_display_cache(
    node: &Node,
    debug: &NodeComputedDebug,
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
            .sample_points(debug.0.analysis_resolution),
    }
}
