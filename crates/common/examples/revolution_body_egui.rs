use common::body::{Meshable, RevolutionAxis, RevolutionMesh};
use common::egui_helpers::{
    draw_depth_wireframe, draw_segment_chart_sized, run_native_app,
    show_scrolled_left_panel_inside, show_validation_status, CameraControls, DirectSegmentConfig,
    MeshProjector,
};
use eframe::egui::{self, Color32};

#[allow(dead_code)]
const DARK_BG: Color32 = Color32::from_rgb(18, 20, 26);

fn main() -> eframe::Result<()> {
    run_native_app(
        "RevolutionBody - Surface Mesh Viewer",
        [1400.0, 900.0],
        None,
        |_cc| RevolutionBodyApp::default(),
    )
}

// ─── app state ───────────────────────────────────────────────────────────────

struct RevolutionBodyApp {
    profile_config_1: DirectSegmentConfig,
    profile_config_2: DirectSegmentConfig,
    axis: RevolutionAxis,
    resolution: usize,
    validation_error: Option<String>,
    camera: CameraControls,
}

impl Default for RevolutionBodyApp {
    fn default() -> Self {
        let mut profile_config_1 = DirectSegmentConfig::default();
        profile_config_1.para_a = 0.15;
        profile_config_1.para_c = 1.0;

        let mut profile_config_2 = DirectSegmentConfig::default();
        profile_config_2.para_a = -0.10;
        profile_config_2.para_c = 1.8;

        Self {
            profile_config_1,
            profile_config_2,
            axis: RevolutionAxis::Y,
            resolution: 16,
            validation_error: None,
            camera: CameraControls::orbit(0.5, 0.7),
        }
    }
}

impl RevolutionBodyApp {
    fn try_build_body(&self) -> Result<RevolutionMesh<2>, String> {
        let seg1 = self
            .profile_config_1
            .build_segment()
            .map_err(|err| format!("segment 1: {err}"))?;

        let mut seg2 = self
            .profile_config_2
            .build_segment()
            .map_err(|err| format!("segment 2: {err}"))?;

        // Chain seg2 to start where seg1 ends (no continuity API)
        seg2.start += seg1.end;
        seg2.end += seg1.end;

        RevolutionMesh::new([seg1, seg2], self.axis)
            .map_err(|err| format!("body construction: {err}"))
    }

    fn get_mesh_stats(&self) -> Option<(usize, usize)> {
        match self.try_build_body() {
            Ok(body) => Some((
                body.sample_points(self.resolution).len(),
                body.mesh_indices(self.resolution).len(),
            )),
            Err(_) => None,
        }
    }

    fn show_controls(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Segment 1")
            .id_salt("revolution_profile_1")
            .default_open(false)
            .show(ui, |ui| {
                self.profile_config_1
                    .show_controls(ui, "Segment 1", "revolution_segment_1_kind");
            });

        egui::CollapsingHeader::new("Segment 2")
            .id_salt("revolution_profile_2")
            .default_open(false)
            .show(ui, |ui| {
                self.profile_config_2
                    .show_controls(ui, "Segment 2", "revolution_segment_2_kind");
            });

        egui::CollapsingHeader::new("Revolution Axis")
            .id_salt("revolution_axis")
            .default_open(false)
            .show(ui, |ui| {
                for axis in [RevolutionAxis::X, RevolutionAxis::Y, RevolutionAxis::Z] {
                    let label = match axis {
                        RevolutionAxis::X => "X",
                        RevolutionAxis::Y => "Y",
                        RevolutionAxis::Z => "Z",
                    };
                    ui.selectable_value(&mut self.axis, axis, label);
                }
            });

        egui::CollapsingHeader::new("Mesh Configuration")
            .id_salt("revolution_mesh_config")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut self.resolution, 4..=48).text("Resolution"));
            });

        self.camera
            .show_collapsing(ui, "Camera Control", "revolution_camera");

        ui.separator();

        // Validation and error display
        match self.try_build_body() {
            Ok(_) => {
                self.validation_error = None;
                show_validation_status(ui, None, "Valid configuration");

                // Show mesh statistics
                ui.separator();
                ui.heading("Mesh Statistics");
                if let Some((vertex_count, triangle_count)) = self.get_mesh_stats() {
                    ui.label(format!("Vertices: {}", vertex_count));
                    ui.label(format!("Triangles: {}", triangle_count));
                }
            }
            Err(err) => {
                self.validation_error = Some(err);
                show_validation_status(ui, self.validation_error.as_deref(), "Valid configuration");
            }
        }
    }
}

// ─── 2d profile preview ──────────────────────────────────────────────────────

fn draw_2d_profile<const N: usize>(ui: &mut egui::Ui, body: &RevolutionMesh<N>) {
    draw_segment_chart_sized(ui, 300.0, Some(body.profile.as_slice()), None);
}

// ─── 3d mesh preview ───────────────────────────────────────────────────────

fn draw_3d_mesh<const N: usize>(
    ui: &mut egui::Ui,
    body: &RevolutionMesh<N>,
    pitch: f64,
    yaw: f64,
    resolution: usize,
) {
    let desired_size = egui::Vec2::new(700.0, 420.0);
    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
    let rect = response.rect;
    let plot_rect = rect.shrink(8.0);

    painter.rect_filled(rect, 6.0, Color32::from_rgb(248, 250, 255));
    painter.rect_stroke(
        plot_rect,
        4.0,
        egui::Stroke::new(1.0_f32, Color32::from_gray(180)),
        egui::StrokeKind::Middle,
    );

    let points = body.sample_points(resolution);
    let indices = body.mesh_indices(resolution);

    if points.is_empty() || indices.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No mesh data",
            egui::FontId::proportional(14.0),
            Color32::DARK_RED,
        );
        return;
    }

    let xyz_points = points.iter().map(|p| (p.x, p.y, p.z)).collect::<Vec<_>>();
    let (_projector, projected) =
        MeshProjector::from_xyz_points(&xyz_points, plot_rect, pitch, yaw, 0.42);
    draw_depth_wireframe(&painter, &projected, &indices);
}

// ─── eframe app ──────────────────────────────────────────────────────────────

impl eframe::App for RevolutionBodyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        show_scrolled_left_panel_inside(ui, "controls", 300.0, Some(400.0), |ui| {
            self.show_controls(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Ok(body) = self.try_build_body() {
                let (stats_vertices, stats_triangles) = self.get_mesh_stats().unwrap_or((0, 0));
                ui.heading("2D Profile Preview");
                ui.group(|ui| {
                    draw_2d_profile(ui, &body);
                });

                ui.separator();
                ui.heading("3D Mesh Preview");
                ui.label("Wireframe render with camera yaw/pitch");
                ui.group(|ui| {
                    draw_3d_mesh(
                        ui,
                        &body,
                        self.camera.pitch,
                        self.camera.yaw,
                        self.resolution,
                    );
                });
                ui.label(format!(
                    "Mesh: {} vertices, {} triangles",
                    stats_vertices, stats_triangles
                ));
            } else {
                ui.vertical(|ui| {
                    ui.add_space(100.0);
                    ui.heading("⚠ Invalid Configuration");
                    ui.label("Fix errors in the left panel to see preview");
                });
            }
        });
        ui.ctx().request_repaint();
    }
}
