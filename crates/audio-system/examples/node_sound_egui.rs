use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use audio_system::egui_testbed::{RuntimeTestbed, RuntimeTransportState};
use audio_system::system::node::{
    jet_modal_component, slide_modal_component, strike_modal_component,
};
use common::body::materials::{Material, Medium};
use common::config::{Node, NodeComputedDebug, NodeModelBuilders};
use common::egui_helpers::{
    draw_xy_line_chart, draw_xy_multi_line_chart_sized, run_native_app, show_action_error_messages,
    show_scrolled_left_panel_inside, StatusTone,
};
use common::Meshable;
use eframe::egui::{self, Color32, RichText};
use fundsp::prelude::*;
use u_num_it::u_num_it;

const WINDOW_TITLE: &str = "Node modal playground";
const WINDOW_SIZE: [f32; 2] = [1520.0, 980.0];
const EXCITE_FOLLOW_SECS: f32 = 0.02;

fn main() -> eframe::Result<()> {
    run_native_app(
        WINDOW_TITLE,
        WINDOW_SIZE,
        Some("audio_system=trace"),
        |_cc| NodeSoundApp::default(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoundTab {
    Strike,
    Slide,
    Jet,
}

impl SoundTab {
    fn label(self) -> &'static str {
        match self {
            Self::Strike => "Strike",
            Self::Slide => "Slide",
            Self::Jet => "Jet",
        }
    }
}

struct PendingAnalysis {
    receiver: Receiver<AnalysisResult>,
}

enum AnalysisResult {
    Ready(ReadyAnalysis),
    Error(String),
}

struct ReadyAnalysis {
    debug: NodeComputedDebug,
    display: DisplayCache,
}

#[derive(Clone)]
struct DisplayCache {
    bowl_profile: Vec<(f64, f64)>,
    clapper_profile: Vec<(f64, f64)>,
    thickness_face_profile: Vec<(f64, f64)>,
    thickness_back_profile: Vec<(f64, f64)>,
}

struct AnalysisInputs {
    builders: NodeModelBuilders,
    bowl_material: Material,
    clapper_material: Material,
    clapper_to_bowl_friction: f64,
    seed: Option<u64>,
    medium: Medium,
    resolution: usize,
    mode_count: usize,
}

struct NodeSoundApp {
    builders: NodeModelBuilders,
    bowl_material: Material,
    clapper_material: Material,
    clapper_to_bowl_friction: f64,
    medium: Medium,
    use_seed: bool,
    seed: u64,
    resolution: usize,
    mode_count: usize,

    pending_analysis: Option<PendingAnalysis>,
    ready_analysis: Option<ReadyAnalysis>,

    runtime_testbed: RuntimeTestbed,

    active_tab: SoundTab,
    excitation: Arc<Shared>,
    excitation_active: bool,

    last_action: Option<String>,
    last_error: Option<String>,
}

impl Default for NodeSoundApp {
    fn default() -> Self {
        Self {
            builders: NodeModelBuilders::default(),
            bowl_material: Material {
                id: "NODE_SOUND_BOWL".to_string(),
                reference_density_kg_per_m3: 8800.0,
                poisson_ratio: 0.34,
                reference_youngs_modulus_mpa: 110_000.0,
                reference_temperature_c: 20.0,
                linear_thermal_expansion_per_c: 18.0e-6,
                dln_e_dtemp_per_c: -3.0e-4,
            },
            clapper_material: Material {
                id: "NODE_SOUND_CLAPPER".to_string(),
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
            resolution: 8,
            mode_count: 8,
            pending_analysis: None,
            ready_analysis: None,
            runtime_testbed: RuntimeTestbed::default(),
            active_tab: SoundTab::Strike,
            excitation: Arc::new(Shared::new(0.0)),
            excitation_active: false,
            last_action: None,
            last_error: None,
        }
    }
}

impl NodeSoundApp {
    fn set_action(&mut self, action: impl ToString) {
        self.last_action = Some(action.to_string());
        self.last_error = None;
    }

    fn set_error(&mut self, error: impl ToString) {
        self.last_error = Some(error.to_string());
    }

    fn analysis_inputs(&self) -> AnalysisInputs {
        AnalysisInputs {
            builders: self.builders.clone(),
            bowl_material: self.bowl_material.clone(),
            clapper_material: self.clapper_material.clone(),
            clapper_to_bowl_friction: self.clapper_to_bowl_friction,
            seed: self.use_seed.then_some(self.seed),
            medium: self.medium.clone(),
            resolution: self.resolution,
            mode_count: self.mode_count,
        }
    }

    fn request_analysis(&mut self) {
        let inputs = self.analysis_inputs();
        self.pending_analysis = Some(spawn_analysis(inputs));
        self.ready_analysis = None;
        self.set_action("Computing node + medium analysis");
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
                self.ready_analysis = Some(ready);
                self.set_action("Analysis ready. Load a modal graph in any tab.");
            }
            AnalysisResult::Error(err) => {
                self.ready_analysis = None;
                self.set_error(err);
            }
        }
    }

    fn mode_count_clamped(&self) -> usize {
        self.mode_count.clamp(1, 16)
    }

    fn set_excitation_gate(&mut self, active: bool) {
        if self.excitation_active == active {
            return;
        }
        self.excitation_active = active;
        self.excitation.set_value(if active { 1.0 } else { 0.0 });
    }

    fn apply_active_network(&mut self) {
        let Some(ready) = &self.ready_analysis else {
            self.set_error("Run analysis first");
            return;
        };

        let debug = ready.debug.clone();
        let mode_count = self.mode_count_clamped();
        let active_tab = self.active_tab;
        let excitation = self.excitation.clone();
        self.runtime_testbed.apply_custom_network(
            format!(
                "Loaded {} modal graph ({} modes)",
                active_tab.label(),
                mode_count
            ),
            move |sample_rate| {
                build_modal_net(active_tab, &debug, mode_count, excitation, sample_rate)
            },
        );
    }

    fn render_left_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Node + Medium");
        ui.add(egui::Slider::new(&mut self.resolution, 2..=96).text("analysis resolution"));
        ui.add(egui::Slider::new(&mut self.mode_count, 1..=16).text("mode count (u_num_it)"));

        if ui.button("Run analysis").clicked() {
            self.request_analysis();
        }

        ui.horizontal(|ui| {
            if self.pending_analysis.is_some() {
                ui.spinner();
                ui.label("analysis in progress");
            } else {
                ui.add_space(18.0);
                ui.label("analysis idle");
            }
        });

        egui::CollapsingHeader::new("Shape")
            .id_salt("node_sound_shape")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.height_m, 0.3..=1.8).text("height"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.radius_0_m, 0.005..=5.45)
                        .text("radius 0"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.radius_1_m, 0.005..=5.55)
                        .text("radius 1"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.radius_2_m, 0.005..=5.65)
                        .text("radius 2"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.radius_3_m, 0.005..=5.65)
                        .text("radius 3"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.radius_4_m, 0.005..=5.65)
                        .text("radius 4"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.radius_5_m, 0.005..=5.65)
                        .text("radius 5"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.profile_bias, -1.0..=1.0)
                        .text("profile bias"),
                );
                ui.add(
                    egui::Slider::new(&mut self.builders.shape.sampling_density, 2.0..=64.0)
                        .text("profile density"),
                );
            });

        egui::CollapsingHeader::new("Clapper")
            .id_salt("node_sound_clapper")
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
                ui.add(
                    egui::Slider::new(&mut self.clapper_to_bowl_friction, 0.0..=1.5)
                        .text("clapper to bowl friction"),
                );
            });

        egui::CollapsingHeader::new("Thickness")
            .id_salt("node_sound_thickness")
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
            .id_salt("node_sound_seed")
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
            .id_salt("node_sound_medium")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.medium.temperature_c, -40.0..=120.0)
                        .text("temperature (C)"),
                );
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
                if ui.button("Recompute impedance = density * c").clicked() {
                    self.medium.impedance_m_rayl = Medium::impedance_from_density_and_speed(
                        self.medium.density_kg_per_m3,
                        self.medium.speed_of_sound_m_per_s,
                    );
                }
                ui.label(format!(
                    "impedance: {:.3} Rayl",
                    self.medium.impedance_m_rayl
                ));
            });

        ui.separator();
        ui.label(RichText::new("Runtime messages").strong());
        self.runtime_testbed.render_runtime_messages(ui);
        ui.separator();
        show_action_error_messages(ui, &self.last_action, &self.last_error);
    }

    fn render_transport_bar(&mut self, ui: &mut egui::Ui) {
        let transport = self.runtime_testbed.transport_state();
        ui.horizontal_wrapped(|ui| {
            ui.label("Status");
            let tone = match transport {
                RuntimeTransportState::Stopped => StatusTone::Idle,
                RuntimeTransportState::Running => StatusTone::Success,
                RuntimeTransportState::Paused => StatusTone::Warning,
            };
            ui.colored_label(tone.color(), transport.label());

            if ui
                .add_enabled(
                    transport == RuntimeTransportState::Stopped,
                    egui::Button::new("Start"),
                )
                .clicked()
            {
                self.runtime_testbed.start_transport();
            }

            if ui
                .add_enabled(
                    transport == RuntimeTransportState::Running,
                    egui::Button::new("Pause"),
                )
                .clicked()
            {
                self.runtime_testbed.pause_transport();
            }

            if ui
                .add_enabled(
                    transport == RuntimeTransportState::Paused,
                    egui::Button::new("Resume"),
                )
                .clicked()
            {
                self.runtime_testbed.resume_transport();
            }

            if ui
                .add_enabled(
                    transport != RuntimeTransportState::Stopped,
                    egui::Button::new("Stop"),
                )
                .clicked()
            {
                self.set_excitation_gate(false);
                self.runtime_testbed.stop_transport();
            }

            if ui.button("Load active tab graph").clicked() {
                self.apply_active_network();
            }
        });

        if self.runtime_testbed.transport_state() == RuntimeTransportState::Stopped {
            self.set_excitation_gate(false);
        }

        ui.horizontal(|ui| {
            let response = ui.add(egui::Button::new("Excite (hold mouse or Space)"));
            let pointer_held = response.is_pointer_button_down_on();
            let key_held = ui.ctx().input(|input| input.key_down(egui::Key::Space));
            self.set_excitation_gate(pointer_held || key_held);

            if self.excitation_active {
                ui.colored_label(StatusTone::Success.color(), "excitation gate: ON");
            } else {
                ui.colored_label(StatusTone::Idle.color(), "excitation gate: OFF");
            }
        });
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tab in [SoundTab::Strike, SoundTab::Slide, SoundTab::Jet] {
                ui.selectable_value(&mut self.active_tab, tab, tab.label());
            }
        });
    }

    fn render_geometry_preview(&self, ui: &mut egui::Ui) {
        ui.heading("Geometry Preview");

        let Some(ready) = &self.ready_analysis else {
            ui.label("Run analysis to see node preview charts.");
            return;
        };

        let display = &ready.display;
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
                190.0,
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
                "y",
                "m",
            );
        });
    }

    fn render_modal_preview(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("{} Modal Preview", self.active_tab.label()));
        ui.label(format!(
            "Mode count uses u_num_it dispatch range 1..=16 (selected: {}).",
            self.mode_count_clamped()
        ));
        ui.separator();
        self.runtime_testbed.render_diagnostics(ui);

        ui.separator();
        self.render_active_tab_mode_table(ui);
    }

    fn render_active_tab_mode_table(&self, ui: &mut egui::Ui) {
        let Some(ready) = &self.ready_analysis else {
            ui.label("Run analysis to inspect modal parameters.");
            return;
        };

        let (structure, acoustics) = &ready.debug;
        let count = self.mode_count_clamped();

        egui::ScrollArea::vertical()
            .id_salt("node_sound_modal_table")
            .max_height(320.0)
            .show(ui, |ui| match self.active_tab {
                SoundTab::Strike => {
                    ui.label(RichText::new("strike_modal_component").strong());
                    for (i, (s_mode, a_mode)) in structure
                        .strike_modes
                        .iter()
                        .zip(acoustics.strike_modes.iter())
                        .take(count)
                        .enumerate()
                    {
                        ui.monospace(format!(
                            "S#{:02} freq={:8.2}Hz damp={:.5} bw={:7.2}Hz coupling={:.3} angle={:.3}",
                            i + 1,
                            a_mode.frequency_hz,
                            a_mode.damping_in_medium,
                            a_mode.impact_bandwidth_hz,
                            s_mode.strike_base.coupling,
                            s_mode.strike_base.angle_sensitivity,
                        ));
                    }
                }
                SoundTab::Slide => {
                    ui.label(RichText::new("slide_modal_component").strong());
                    for (i, (s_mode, a_mode)) in structure
                        .slide_modes
                        .iter()
                        .zip(acoustics.slide_modes.iter())
                        .take(count)
                        .enumerate()
                    {
                        ui.monospace(format!(
                            "L#{:02} freq={:8.2}Hz damp={:.5} bw={:7.2}Hz gain={:.3} squeal={:.3} coupling={:.3}",
                            i + 1,
                            a_mode.frequency_hz,
                            a_mode.damping_in_medium,
                            a_mode.slide_bandwidth_hz,
                            a_mode.friction_interaction_gain,
                            a_mode.squeal_tendency,
                            s_mode.slide_base.coupling,
                        ));
                    }
                }
                SoundTab::Jet => {
                    ui.label(RichText::new("jet_modal_component").strong());
                    ui.monospace(format!(
                        "source mode={} freq={:8.2}Hz damp={:.5} lock={:8.2}Hz bw={:7.2}Hz phase={:.3}",
                        structure.jet_mode.source_mode_index + 1,
                        acoustics.jet_mode.frequency_hz,
                        acoustics.jet_mode.damping_in_medium,
                        acoustics.jet_mode.acoustic_lock_in.lock_center_hz,
                        acoustics.jet_mode.acoustic_lock_in.lock_bandwidth_hz,
                        acoustics.jet_mode.acoustic_lock_in.phase_sensitivity,
                    ));
                }
            });
    }
}

impl eframe::App for NodeSoundApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_analysis();

        show_scrolled_left_panel_inside(ui, "node_sound_controls", 360.0, Some(430.0), |ui| {
            self.render_left_controls(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_tabs(ui);
            ui.separator();
            self.render_transport_bar(ui);
            ui.separator();

            ui.group(|ui| {
                self.render_geometry_preview(ui);
            });

            ui.add_space(8.0);
            ui.group(|ui| {
                self.render_modal_preview(ui);
            });
        });

        if self.pending_analysis.is_some()
            || self.runtime_testbed.transport_state() != RuntimeTransportState::Stopped
        {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
    }
}

fn spawn_analysis(inputs: AnalysisInputs) -> PendingAnalysis {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let result = run_analysis(inputs);
        let _ = sender.send(result);
    });

    PendingAnalysis { receiver }
}

fn run_analysis(inputs: AnalysisInputs) -> AnalysisResult {
    let node = match inputs.builders.build_node(
        inputs.bowl_material,
        inputs.clapper_material,
        inputs.clapper_to_bowl_friction,
        inputs.seed,
    ) {
        Ok(node) => node,
        Err(err) => return AnalysisResult::Error(err),
    };

    let structure = match node.computed_structure(inputs.resolution, inputs.mode_count) {
        Some(structure) => structure,
        None => return AnalysisResult::Error("computed structure unavailable".to_string()),
    };

    let acoustics = match node.computed_acoustics_from_structure(&structure, &inputs.medium) {
        Some(acoustics) => acoustics,
        None => return AnalysisResult::Error("computed acoustics unavailable".to_string()),
    };

    let display = build_display_cache(
        &node,
        inputs.builders.thickness.inner_base_thickness_m,
        inputs.builders.thickness.inner_lip_thickness_m,
        inputs.builders.thickness.outer_base_thickness_m,
        inputs.builders.thickness.outer_lip_thickness_m,
    );

    AnalysisResult::Ready(ReadyAnalysis {
        debug: (structure, acoustics),
        display,
    })
}

fn build_display_cache(
    node: &Node,
    inner_base_thickness_m: f64,
    inner_lip_thickness_m: f64,
    outer_base_thickness_m: f64,
    outer_lip_thickness_m: f64,
) -> DisplayCache {
    let bowl_mesh = node.bowl.surface_mesh(80);
    let clapper_mesh = node.clapper.surface_mesh(80);
    let shell_mesh = node.bowl.meshable.surface_mesh_data(40);

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
        bowl_profile: radial_profile_from_xyz(
            &bowl_mesh
                .0
                .iter()
                .map(|p| (p.x, p.y, p.z))
                .collect::<Vec<_>>(),
            80,
        ),
        clapper_profile: radial_profile_from_xyz(
            &clapper_mesh
                .0
                .iter()
                .map(|p| (p.x, p.y, p.z))
                .collect::<Vec<_>>(),
            80,
        ),
        thickness_face_profile,
        thickness_back_profile,
    }
}

fn radial_profile_from_xyz(points: &[(f64, f64, f64)], bins: usize) -> Vec<(f64, f64)> {
    if points.is_empty() || bins < 2 {
        return vec![];
    }

    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in points {
        min_y = min_y.min(point.1);
        max_y = max_y.max(point.1);
    }
    if !min_y.is_finite() || !max_y.is_finite() {
        return vec![];
    }
    let span = (max_y - min_y).max(1.0e-12);

    let mut max_radius_by_bin = vec![0.0f64; bins];
    let mut has_point = vec![false; bins];
    for point in points {
        let t = ((point.1 - min_y) / span).clamp(0.0, 1.0);
        let index = std::cmp::min((t * (bins - 1) as f64).round() as usize, bins - 1);
        let radius = (point.0 * point.0 + point.2 * point.2).sqrt();
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

fn build_modal_net(
    tab: SoundTab,
    debug: &NodeComputedDebug,
    mode_count: usize,
    excitation: Arc<Shared>,
    sample_rate: f64,
) -> Result<Net, String> {
    let modal_id_and_net = match tab {
        SoundTab::Strike => build_strike_net(debug, mode_count, excitation),
        SoundTab::Slide => build_slide_net(debug, mode_count, excitation),
        SoundTab::Jet => build_jet_net(debug, excitation),
    }?;

    let mut net = Net::new(1, 2);
    let sink_id = net.push(Box::new(sink()));
    let modal_id = net.push(modal_id_and_net);

    net.connect_input(0, sink_id, 0);
    net.connect_output(modal_id, 0, 0);
    net.connect_output(modal_id, 1, 1);
    net.set_sample_rate(sample_rate);
    net.check();

    Ok(net)
}

fn build_strike_net(
    debug: &NodeComputedDebug,
    mode_count: usize,
    excitation: Arc<Shared>,
) -> Result<Box<dyn AudioUnit>, String> {
    let (structure, acoustics) = debug;
    let count = std::cmp::Ord::min(
        mode_count,
        std::cmp::Ord::min(structure.strike_modes.len(), acoustics.strike_modes.len()),
    );

    if count == 0 {
        return Err("No strike modes available".to_string());
    }
    if count > 16 {
        return Err("mode count must be in 1..=16 for strike".to_string());
    }

    let modes = &acoustics.strike_modes[..count];
    let mode_structures = &structure.strike_modes[..count];

    Ok(u_num_it!(
        1..=16,
        match count {
            U => {
                let gate = var(&excitation); // >> follow(EXCITE_FOLLOW_SECS);
                Box::new(
                    gate >> strike_modal_component::<f32, NumType>(modes, mode_structures)
                        >> split::<U2>(),
                ) as Box<dyn AudioUnit>
            }
        }
    ))
}

fn build_slide_net(
    debug: &NodeComputedDebug,
    mode_count: usize,
    excitation: Arc<Shared>,
) -> Result<Box<dyn AudioUnit>, String> {
    let (structure, acoustics) = debug;
    let count = std::cmp::Ord::min(
        mode_count,
        std::cmp::Ord::min(structure.slide_modes.len(), acoustics.slide_modes.len()),
    );

    if count == 0 {
        return Err("No slide modes available".to_string());
    }
    if count > 16 {
        return Err("mode count must be in 1..=16 for slide".to_string());
    }

    let modes = &acoustics.slide_modes[..count];
    let mode_structures = &structure.slide_modes[..count];

    Ok(u_num_it!(
        1..=16,
        match count {
            U => {
                let gate = var(&excitation) >> follow(EXCITE_FOLLOW_SECS);
                Box::new(
                    gate >> slide_modal_component::<f32, NumType>(modes, mode_structures)
                        >> split::<U2>(),
                ) as Box<dyn AudioUnit>
            }
        }
    ))
}

fn build_jet_net(
    debug: &NodeComputedDebug,
    excitation: Arc<Shared>,
) -> Result<Box<dyn AudioUnit>, String> {
    let (structure, acoustics) = debug;
    let lock_center_hz = acoustics.jet_mode.acoustic_lock_in.lock_center_hz as f32;
    let gate = var(&excitation) >> follow(EXCITE_FOLLOW_SECS);

    Ok(Box::new(
        (gate * sine_hz::<f32>(lock_center_hz))
            >> jet_modal_component::<f32>(&acoustics.jet_mode, &structure.jet_mode)
            >> split::<U2>(),
    ) as Box<dyn AudioUnit>)
}
