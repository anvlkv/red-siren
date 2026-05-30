use common::body::{
    EmbodiedPoint3, EmbodiedTriangle, Meshable, RevolutionAxis, RevolutionMesh, ThickMesh,
    ThicknessMap, ThicknessMapPoint,
};
use common::egui_helpers::{
    draw_depth_wireframe, draw_segment_chart_sized, draw_xy_multi_line_chart_sized, run_native_app,
    show_scrolled_left_panel_inside, show_validation_status, CameraControls, CurveKind,
    DirectSegmentConfig, MeshProjector,
};
use eframe::egui::{self, Color32, Shape, Stroke};
use nalgebra::Vector3;

#[derive(Clone, Copy, PartialEq, Debug)]
enum ProbeMode {
    Normal,
    Manual,
}

fn main() -> eframe::Result<()> {
    run_native_app(
        "Thick RevolutionBody - Surface Mesh Viewer",
        [1450.0, 920.0],
        |_cc| ThickRevolutionBodyApp::default(),
    )
}

struct ThickRevolutionBodyApp {
    profile_config_1: DirectSegmentConfig,
    profile_config_2: DirectSegmentConfig,
    face_thickness_config: DirectSegmentConfig,
    backface_thickness_config: DirectSegmentConfig,
    axis: RevolutionAxis,
    resolution: usize,
    validation_error: Option<String>,
    camera: CameraControls,
    selected_vertex_index: usize,
    probe_mode: ProbeMode,
    manual_direction: [f64; 3],
}

impl Default for ThickRevolutionBodyApp {
    fn default() -> Self {
        let mut profile_config_1 = DirectSegmentConfig::default();
        profile_config_1.para_a = 0.12;
        profile_config_1.para_c = 1.0;

        let mut profile_config_2 = DirectSegmentConfig::default();
        profile_config_2.para_a = -0.10;
        profile_config_2.para_c = 1.9;

        let mut face_thickness_config = DirectSegmentConfig::default();
        face_thickness_config.start = 0.0;
        face_thickness_config.end = 1.0;
        face_thickness_config.kind = CurveKind::Parabolic;
        face_thickness_config.para_a = -0.10;
        face_thickness_config.para_b = 0.0;
        face_thickness_config.para_c = 0.08;

        let mut backface_thickness_config = DirectSegmentConfig::default();
        backface_thickness_config.start = 0.0;
        backface_thickness_config.end = 1.0;
        backface_thickness_config.kind = CurveKind::Parabolic;
        backface_thickness_config.para_a = 0.08;
        backface_thickness_config.para_b = 0.0;
        backface_thickness_config.para_c = 0.05;

        Self {
            profile_config_1,
            profile_config_2,
            face_thickness_config,
            backface_thickness_config,
            axis: RevolutionAxis::Y,
            resolution: 16,
            validation_error: None,
            camera: CameraControls::orbit(0.5, 0.7),
            selected_vertex_index: 0,
            probe_mode: ProbeMode::Normal,
            manual_direction: [0.0, -1.0, 0.0],
        }
    }
}

impl ThickRevolutionBodyApp {
    fn try_build_scene(&self) -> Result<(RevolutionMesh<2>, ThickMesh<RevolutionMesh<2>>), String> {
        let seg1 = self
            .profile_config_1
            .build_segment()
            .map_err(|err| format!("profile segment 1: {err}"))?;

        let mut seg2 = self
            .profile_config_2
            .build_segment()
            .map_err(|err| format!("profile segment 2: {err}"))?;

        seg2.start += seg1.end;
        seg2.end += seg1.end;

        let body = RevolutionMesh::new([seg1, seg2], self.axis)
            .map_err(|err| format!("body construction: {err}"))?;

        let face_segment = self
            .face_thickness_config
            .build_segment()
            .map_err(|err| format!("face thickness segment: {err}"))?;

        let backface_segment = self
            .backface_thickness_config
            .build_segment()
            .map_err(|err| format!("backface thickness segment: {err}"))?;

        let face_span = face_segment.end - face_segment.start;
        let back_span = backface_segment.end - backface_segment.start;
        if face_span.abs() < 1e-12 || back_span.abs() < 1e-12 {
            return Err("thickness segment span must be non-zero".to_string());
        }

        let axial_positions = body.profile_sample_positions(self.resolution);
        let mut thickness_points = Vec::with_capacity(axial_positions.len() * self.resolution);
        for j in 0..self.resolution {
            for (i, u) in axial_positions.iter().enumerate() {
                let face_t = face_segment.start + u * face_span;
                let backface_t = backface_segment.start + u * back_span;
                let face = face_segment.evaluate(face_t).max(0.0);
                let backface = backface_segment.evaluate(backface_t).max(0.0);
                thickness_points.push(ThicknessMapPoint {
                    body_vertex_index: j * axial_positions.len() + i,
                    face_thickness: face,
                    backface_thickness: backface,
                });
            }
        }
        let thickness_map = ThicknessMap::new(thickness_points)
            .ok_or_else(|| "thickness map construction failed (invalid values)".to_string())?;

        Ok((body.clone(), ThickMesh::new(body, thickness_map)))
    }

    fn sample_thickness_curves(
        &self,
        body: &RevolutionMesh<2>,
        samples: usize,
    ) -> Result<Vec<(f64, f64, f64)>, String> {
        if samples < 2 {
            return Ok(vec![]);
        }

        let face_segment = self
            .face_thickness_config
            .build_segment()
            .map_err(|err| format!("face thickness segment: {err}"))?;
        let backface_segment = self
            .backface_thickness_config
            .build_segment()
            .map_err(|err| format!("backface thickness segment: {err}"))?;

        let face_span = face_segment.end - face_segment.start;
        let back_span = backface_segment.end - backface_segment.start;
        if face_span.abs() < 1e-12 || back_span.abs() < 1e-12 {
            return Err("thickness segment span must be non-zero".to_string());
        }

        let (min_t, max_t) = body.height_range();
        let height_span = max_t - min_t;

        let mut out = Vec::with_capacity(samples);
        for i in 0..samples {
            let u = i as f64 / (samples - 1) as f64;
            let face_t = face_segment.start + u * face_span;
            let backface_t = backface_segment.start + u * back_span;
            let face = face_segment.evaluate(face_t).max(0.0);
            let backface = backface_segment.evaluate(backface_t).max(0.0);
            let h = min_t + u * height_span;
            out.push((h, face, backface));
        }

        Ok(out)
    }

    fn requested_probe_direction(
        &self,
        surface_normal: Option<Vector3<f64>>,
    ) -> Option<Vector3<f64>> {
        match self.probe_mode {
            ProbeMode::Normal => surface_normal,
            ProbeMode::Manual => Vector3::new(
                self.manual_direction[0],
                self.manual_direction[1],
                self.manual_direction[2],
            )
            .try_normalize(1e-9),
        }
    }

    fn show_controls(&mut self, ui: &mut egui::Ui) {
        let scene = self.try_build_scene();
        let vertex_count = scene
            .as_ref()
            .ok()
            .map(|(_, thick_body)| thick_body.sample_points(self.resolution).len())
            .unwrap_or(0);
        let max_vertex_index = vertex_count.saturating_sub(1);

        if vertex_count > 0 {
            self.selected_vertex_index = self.selected_vertex_index.min(max_vertex_index);
        } else {
            self.selected_vertex_index = 0;
        }

        egui::CollapsingHeader::new("Profile Segment 1")
            .id_salt("profile_1")
            .default_open(false)
            .show(ui, |ui| {
                self.profile_config_1.show_controls(
                    ui,
                    "Profile Segment 1",
                    "thick_profile_segment_1_kind",
                );
            });

        egui::CollapsingHeader::new("Profile Segment 2")
            .id_salt("profile_2")
            .default_open(false)
            .show(ui, |ui| {
                self.profile_config_2.show_controls(
                    ui,
                    "Profile Segment 2",
                    "thick_profile_segment_2_kind",
                );
            });

        egui::CollapsingHeader::new("Face Thickness Segment")
            .id_salt("face_thickness")
            .default_open(false)
            .show(ui, |ui| {
                self.face_thickness_config.show_controls(
                    ui,
                    "Face Thickness Segment",
                    "thick_face_thickness_kind",
                );
            });

        egui::CollapsingHeader::new("Backface Thickness Segment")
            .id_salt("backface_thickness")
            .default_open(false)
            .show(ui, |ui| {
                self.backface_thickness_config.show_controls(
                    ui,
                    "Backface Thickness Segment",
                    "thick_backface_thickness_kind",
                );
            });

        egui::CollapsingHeader::new("Revolution Axis")
            .id_salt("axis")
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
            .id_salt("mesh_config")
            .default_open(false)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut self.resolution, 4..=48).text("Resolution"));
            });

        self.camera.show_collapsing(ui, "Camera Control", "camera");

        egui::CollapsingHeader::new("Vertex Analysis")
            .id_salt("vertex_analysis")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(format!("Available Vertices: {}", vertex_count));
                ui.add_enabled(
                    vertex_count > 0,
                    egui::Slider::new(&mut self.selected_vertex_index, 0..=max_vertex_index)
                        .step_by(1.0)
                        .text("Vertex Index"),
                );

                ui.separator();
                ui.label("Probe Direction:");

                ui.selectable_value(
                    &mut self.probe_mode,
                    ProbeMode::Normal,
                    "Use Surface Normal",
                );
                ui.selectable_value(&mut self.probe_mode, ProbeMode::Manual, "Manual Direction");

                if self.probe_mode == ProbeMode::Manual {
                    ui.add(
                        egui::Slider::new(&mut self.manual_direction[0], -1.0..=1.0).text("Dir X"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.manual_direction[1], -1.0..=1.0).text("Dir Y"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.manual_direction[2], -1.0..=1.0).text("Dir Z"),
                    );
                }
            });

        ui.separator();

        match scene {
            Ok((_, _thick_body)) => {
                self.validation_error = None;
                show_validation_status(ui, None, "Valid configuration");
            }
            Err(err) => {
                self.validation_error = Some(err);
                show_validation_status(ui, self.validation_error.as_deref(), "Valid configuration");
            }
        }
    }
}

fn draw_2d_profile<const N: usize>(ui: &mut egui::Ui, body: &RevolutionMesh<N>) {
    draw_segment_chart_sized(ui, 280.0, Some(body.profile.as_slice()), None);
}

fn draw_2d_thickness_preview(ui: &mut egui::Ui, samples: &[(f64, f64, f64)]) {
    let face = samples.iter().map(|(t, f, _)| (*t, *f)).collect::<Vec<_>>();
    let back = samples.iter().map(|(t, _, b)| (*t, *b)).collect::<Vec<_>>();
    let series: [(&str, &[(f64, f64)], Color32); 2] = [
        ("face", face.as_slice(), Color32::from_rgb(216, 69, 69)),
        ("backface", back.as_slice(), Color32::from_rgb(55, 121, 214)),
    ];
    draw_xy_multi_line_chart_sized(ui, "Thickness", 220.0, &series, "h", "m");
}

fn draw_3d_mesh<M>(
    ui: &mut egui::Ui,
    mesh: &M,
    pitch: f64,
    yaw: f64,
    resolution: usize,
    selected_vertex: Option<usize>,
    selected_probe_direction: Option<Vector3<f64>>,
) where
    M: Meshable<Vertex = EmbodiedPoint3, Index = EmbodiedTriangle>,
{
    let desired_size = egui::Vec2::new(760.0, 460.0);
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

    let points = mesh.sample_points(resolution);
    let indices = mesh.mesh_indices(resolution);

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
    let (projector, projected) =
        MeshProjector::from_xyz_points(&xyz_points, plot_rect, pitch, yaw, 0.42);

    let mut face_data: Vec<([usize; 3], f64, Color32)> = Vec::with_capacity(indices.len());

    let outer_fill = Color32::from_rgba_unmultiplied(226, 121, 97, 130);
    let inner_fill = Color32::from_rgba_unmultiplied(84, 154, 236, 130);
    let rim_fill = Color32::from_rgba_unmultiplied(120, 186, 132, 118);

    for tri in &indices {
        let ai = tri[0] as usize;
        let bi = tri[1] as usize;
        let ci = tri[2] as usize;
        if ai >= projected.len() || bi >= projected.len() || ci >= projected.len() {
            continue;
        }

        let depth = (projected[ai].1 + projected[bi].1 + projected[ci].1) / 3.0;

        let color = if ai % 2 == 0 && bi % 2 == 0 && ci % 2 == 0 {
            outer_fill
        } else if ai % 2 == 1 && bi % 2 == 1 && ci % 2 == 1 {
            inner_fill
        } else {
            rim_fill
        };
        face_data.push(([ai, bi, ci], depth, color));
    }

    face_data.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    for (tri, _depth, color) in face_data {
        painter.add(Shape::convex_polygon(
            vec![
                projected[tri[0]].0,
                projected[tri[1]].0,
                projected[tri[2]].0,
            ],
            color,
            Stroke::NONE,
        ));
    }
    draw_depth_wireframe(&painter, &projected, &indices);

    // Draw selected vertex highlight
    if let Some(selected_idx) = selected_vertex {
        if selected_idx < projected.len() {
            if let Some(probe_direction) = selected_probe_direction {
                let start_world = points[selected_idx];
                let normal_length = 0.18 * projector.max_extent().max(1.0);
                let end_world = start_world + probe_direction * normal_length;
                let start_screen = projected[selected_idx].0;
                let end_screen = projector
                    .project_xyz(end_world.x, end_world.y, end_world.z)
                    .0;

                painter.line_segment(
                    [start_screen, end_screen],
                    Stroke::new(1.5_f32, Color32::from_rgb(255, 196, 64)),
                );
                painter.circle_filled(end_screen, 2.0_f32, Color32::from_rgb(255, 196, 64));
            }

            let vertex_pos = projected[selected_idx].0;
            let radius = 3.5_f32;
            painter.circle_filled(vertex_pos, radius, Color32::YELLOW);
            painter.circle_stroke(vertex_pos, radius, Stroke::new(1.25_f32, Color32::GOLD));
        }
    }
}

impl eframe::App for ThickRevolutionBodyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        show_scrolled_left_panel_inside(ui, "controls", 320.0, Some(430.0), |ui| {
            self.show_controls(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| match self.try_build_scene() {
            Ok((body, thick_body)) => {
                let selected_normal = thick_body
                    .surface_normal_at_vertex(self.resolution, self.selected_vertex_index);
                let requested_probe_direction = self.requested_probe_direction(selected_normal);
                let (display_probe_direction, thickness_result) = match requested_probe_direction {
                    Some(direction) => {
                        let forward = thick_body.material_thickness_at_vertex(
                            self.resolution,
                            self.selected_vertex_index,
                            direction,
                        );
                        if self.probe_mode == ProbeMode::Normal {
                            match forward {
                                Some(thickness) => (Some(direction), Some(thickness)),
                                None => {
                                    let reverse_direction = -direction;
                                    match thick_body.material_thickness_at_vertex(
                                        self.resolution,
                                        self.selected_vertex_index,
                                        reverse_direction,
                                    ) {
                                        Some(thickness) => {
                                            (Some(reverse_direction), Some(thickness))
                                        }
                                        None => (Some(direction), None),
                                    }
                                }
                            }
                        } else {
                            (Some(direction), forward)
                        }
                    }
                    None => (None, None),
                };

                let points = thick_body.sample_points(self.resolution);
                let stats_vertices = points.len();
                let stats_triangles = thick_body.mesh_indices(self.resolution).len();
                ui.columns(2, |cols| {
                    cols[0].heading("2D Profile Preview");
                    cols[0].group(|ui| {
                        draw_2d_profile(ui, &body);
                    });

                    cols[1].heading("2D Thickness Preview");
                    cols[1].group(|ui| match self.sample_thickness_curves(&body, 96) {
                        Ok(samples) => draw_2d_thickness_preview(ui, &samples),
                        Err(err) => {
                            ui.colored_label(
                                Color32::LIGHT_RED,
                                format!("Thickness preview error: {}", err),
                            );
                        }
                    });
                });

                ui.separator();
                ui.heading(format!(
                    "3D Thick Mesh Preview | {} vertices, {} triangles",
                    stats_vertices, stats_triangles
                ));
                ui.label(
                    "Filled render: outside (warm), inside (cool), rim (green) + wireframe overlay; selected vertex in yellow",
                );

                ui.columns(2, |cols| {
                    cols[0].group(|ui| {
                        draw_3d_mesh(
                            ui,
                            &thick_body,
                            self.camera.pitch,
                            self.camera.yaw,
                            self.resolution,
                            Some(self.selected_vertex_index),
                            display_probe_direction,
                        );
                    });

                    cols[1].group(|ui| {
                        ui.heading("Embodied Stats");

                        let mat_vol = thick_body.material_volume_m3(self.resolution);
                        ui.label(format!("Material Volume: {:.6} m³", mat_vol));

                        match thick_body.cavity_volume_m3(self.resolution) {
                            Some(cav_vol) => {
                                ui.label(format!("Cavity Volume: {:.6} m³", cav_vol));
                            }
                            None => {
                                ui.label("Cavity Volume: (unavailable)");
                            }
                        }

                        ui.separator();
                        ui.heading("Vertex Probes");
                        ui.label(format!("Selected Index: {}", self.selected_vertex_index));

                        if self.selected_vertex_index < points.len() {
                            let pos = points[self.selected_vertex_index];
                            ui.label(format!("Pos X: {:.6}", pos.x));
                            ui.label(format!("Pos Y: {:.6}", pos.y));
                            ui.label(format!("Pos Z: {:.6}", pos.z));

                            ui.separator();

                            match selected_normal {
                                Some(normal) => {
                                    ui.label(format!("Normal X: {:.6}", normal.x));
                                    ui.label(format!("Normal Y: {:.6}", normal.y));
                                    ui.label(format!("Normal Z: {:.6}", normal.z));
                                }
                                None => {
                                    ui.colored_label(Color32::LIGHT_RED, "Normal: (invalid vertex)");
                                }
                            }

                            ui.separator();

                            if let Some(dir) = display_probe_direction {
                                ui.label(format!("Probe Dir X: {:.6}", dir.x));
                                ui.label(format!("Probe Dir Y: {:.6}", dir.y));
                                ui.label(format!("Probe Dir Z: {:.6}", dir.z));

                                match thickness_result {
                                    Some(thickness) => {
                                        ui.label(format!("Thickness: {:.6} m", thickness));
                                    }
                                    None => {
                                        ui.colored_label(Color32::LIGHT_RED, "Thickness: (no hit)");
                                    }
                                }
                            } else {
                                ui.colored_label(Color32::LIGHT_RED, "Thickness: (invalid direction)");
                            }
                        } else {
                            ui.colored_label(Color32::LIGHT_RED, "Vertex index out of bounds");
                        }
                    });
                });
            }
            Err(_) => {
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
