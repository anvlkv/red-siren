use common::geometry::{Embodied, RevolutionAxis, RevolutionBody, Segment};
use eframe::egui::{self, Color32};

#[allow(dead_code)]
const DARK_BG: Color32 = Color32::from_rgb(18, 20, 26);

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RevolutionBody - Surface Mesh Viewer")
            .with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RevolutionBody - Surface Mesh Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(RevolutionBodyApp::default()))),
    )
}

// ─── curve kind selector ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum CurveKind {
    Line,
    Constant,
    Parabolic,
    Hyperbolic,
    ArcSweep,
}

impl CurveKind {
    fn label(self) -> &'static str {
        match self {
            Self::Line => "Line",
            Self::Constant => "Constant",
            Self::Parabolic => "Parabolic",
            Self::Hyperbolic => "Hyperbolic",
            Self::ArcSweep => "ArcSweep",
        }
    }
}

// ─── profile config ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
struct DirectSegmentConfig {
    start: f64,
    end: f64,
    density: f64,
    kind: CurveKind,
    line_m: f64,
    line_b: f64,
    const_value: f64,
    para_a: f64,
    para_b: f64,
    para_c: f64,
    hyp_a: f64,
    hyp_b: f64,
    hyp_c: f64,
    arc_center_y: f64,
    arc_radius: f64,
    arc_start_angle: f64,
    arc_sweep_angle: f64,
}

impl Default for DirectSegmentConfig {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 4.0,
            density: 16.0,
            kind: CurveKind::Parabolic,
            line_m: 1.0,
            line_b: 0.0,
            const_value: 1.5,
            para_a: 0.25,
            para_b: 0.0,
            para_c: 0.5,
            hyp_a: 1.0,
            hyp_b: -1.0,
            hyp_c: 0.0,
            arc_center_y: 0.0,
            arc_radius: 2.0,
            arc_start_angle: 0.0,
            arc_sweep_angle: std::f64::consts::FRAC_PI_2,
        }
    }
}

impl DirectSegmentConfig {
    fn build_segment(&self) -> Result<Segment, String> {
        match self.kind {
            CurveKind::Line => Segment::start_line(self.end - self.start, self.line_m, self.line_b),
            CurveKind::Constant => Segment::start_constant(self.end - self.start, self.const_value),
            CurveKind::Parabolic => Segment::start_parabolic(
                self.end - self.start,
                self.para_a,
                self.para_b,
                self.para_c,
            ),
            CurveKind::Hyperbolic => {
                Segment::start_hyperbolic(self.end - self.start, self.hyp_a, self.hyp_b, self.hyp_c)
            }
            CurveKind::ArcSweep => Segment::start_arc_sweep(
                self.end - self.start,
                self.arc_center_y,
                self.arc_radius,
                self.arc_start_angle,
                self.arc_sweep_angle,
            ),
        }
        .map_err(|err| err.to_string())
    }

    fn show_controls(&mut self, ui: &mut egui::Ui, heading: &str) {
        ui.heading(heading);
        ui.label("Domain");
        ui.add(egui::Slider::new(&mut self.start, -10.0..=10.0).text("start"));
        ui.add(egui::Slider::new(&mut self.end, -10.0..=10.0).text("end"));
        ui.add(egui::Slider::new(&mut self.density, 1.0..=64.0).text("sampling density"));

        ui.separator();
        ui.label("Curve type");
        egui::ComboBox::from_id_salt(format!("{}_kind", heading))
            .selected_text(self.kind.label())
            .show_ui(ui, |ui| {
                for k in [
                    CurveKind::Line,
                    CurveKind::Constant,
                    CurveKind::Parabolic,
                    CurveKind::Hyperbolic,
                    CurveKind::ArcSweep,
                ] {
                    ui.selectable_value(&mut self.kind, k, k.label());
                }
            });

        ui.separator();
        ui.label("Parameters");
        match self.kind {
            CurveKind::Line => {
                ui.add(egui::Slider::new(&mut self.line_m, -10.0..=10.0).text("m (slope)"));
                ui.add(egui::Slider::new(&mut self.line_b, -10.0..=10.0).text("b (intercept)"));
            }
            CurveKind::Constant => {
                ui.add(egui::Slider::new(&mut self.const_value, -10.0..=10.0).text("value"));
            }
            CurveKind::Parabolic => {
                ui.add(egui::Slider::new(&mut self.para_a, -5.0..=5.0).text("a"));
                ui.add(egui::Slider::new(&mut self.para_b, -10.0..=10.0).text("b"));
                ui.add(egui::Slider::new(&mut self.para_c, -10.0..=10.0).text("c"));
            }
            CurveKind::Hyperbolic => {
                ui.add(egui::Slider::new(&mut self.hyp_a, -5.0..=5.0).text("a"));
                ui.add(
                    egui::DragValue::new(&mut self.hyp_b)
                        .speed(0.05)
                        .prefix("b (pole) "),
                );
                ui.add(egui::Slider::new(&mut self.hyp_c, -10.0..=10.0).text("c"));
                if self.hyp_b > self.start && self.hyp_b < self.end {
                    ui.colored_label(
                        Color32::from_rgb(220, 180, 60),
                        "b (pole) must be outside [start, end]",
                    );
                }
            }
            CurveKind::ArcSweep => {
                ui.add(egui::Slider::new(&mut self.arc_center_y, -10.0..=10.0).text("center_y"));
                ui.add(egui::Slider::new(&mut self.arc_radius, 0.05..=10.0).text("radius"));
                ui.add(
                    egui::Slider::new(
                        &mut self.arc_start_angle,
                        -std::f64::consts::PI..=std::f64::consts::PI,
                    )
                    .text("start_angle (rad)"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.arc_sweep_angle,
                        -std::f64::consts::TAU..=std::f64::consts::TAU,
                    )
                    .text("sweep_angle (rad)"),
                );
            }
        }
    }
}

// ─── app state ───────────────────────────────────────────────────────────────

struct RevolutionBodyApp {
    profile_config_1: DirectSegmentConfig,
    profile_config_2: DirectSegmentConfig,
    axis: RevolutionAxis,
    resolution: usize,
    validation_error: Option<String>,
    camera_pitch: f64,
    camera_yaw: f64,
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
            camera_pitch: 0.5,
            camera_yaw: 0.7,
        }
    }
}

impl RevolutionBodyApp {
    fn try_build_body(&self) -> Result<RevolutionBody<2>, String> {
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

        RevolutionBody::new([seg1, seg2], self.axis)
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
        self.profile_config_1.show_controls(ui, "Segment 1");

        ui.separator();
        self.profile_config_2.show_controls(ui, "Segment 2");

        ui.separator();
        ui.heading("Revolution Axis");
        for axis in [RevolutionAxis::X, RevolutionAxis::Y, RevolutionAxis::Z] {
            let label = match axis {
                RevolutionAxis::X => "X",
                RevolutionAxis::Y => "Y",
                RevolutionAxis::Z => "Z",
            };
            ui.selectable_value(&mut self.axis, axis, label);
        }

        ui.separator();
        ui.heading("Mesh Configuration");
        ui.add(egui::Slider::new(&mut self.resolution, 4..=48).text("Resolution"));

        ui.separator();
        ui.heading("Camera Control");
        ui.add(
            egui::Slider::new(
                &mut self.camera_pitch,
                -std::f64::consts::PI..=std::f64::consts::PI,
            )
            .text("Pitch"),
        );
        ui.add(
            egui::Slider::new(
                &mut self.camera_yaw,
                -std::f64::consts::PI..=std::f64::consts::PI,
            )
            .text("Yaw"),
        );

        ui.separator();

        // Validation and error display
        match self.try_build_body() {
            Ok(_) => {
                ui.colored_label(Color32::GREEN, "✓ Valid configuration");
                self.validation_error = None;

                // Show mesh statistics
                ui.separator();
                ui.heading("Mesh Statistics");
                if let Some((vertex_count, triangle_count)) = self.get_mesh_stats() {
                    ui.label(format!("Vertices: {}", vertex_count));
                    ui.label(format!("Triangles: {}", triangle_count));
                }
            }
            Err(err) => {
                ui.colored_label(Color32::LIGHT_RED, format!("✗ {}", err));
                self.validation_error = Some(err);
            }
        }
    }
}

// ─── 2d profile preview ──────────────────────────────────────────────────────

fn draw_2d_profile<const N: usize>(
    ui: &mut egui::Ui,
    body: &RevolutionBody<N>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (min_t, max_t) = body.height_range();

    let profile_samples = body.sampled_profile_points(64);
    let max_r = profile_samples
        .iter()
        .map(|(_, r)| *r)
        .fold(0.0f64, f64::max);

    // Create a plot area
    let desired_size = egui::Vec2::new(400.0, 300.0);
    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());

    let rect = response.rect;
    let plot_rect = rect.shrink(10.0);

    // Draw background
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    painter.rect_stroke(
        plot_rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
        egui::StrokeKind::Middle,
    );

    // Calculate scaling
    let margin_r = (max_r * 0.15).max(0.2);
    let margin_t = ((max_t - min_t) * 0.1).max(0.5);
    let min_t_scaled = min_t - margin_t;
    let max_t_scaled = max_t + margin_t;
    let min_r_scaled = 0.0;
    let max_r_scaled = max_r + margin_r;

    let width = plot_rect.width() as f64;
    let height = plot_rect.height() as f64;

    // Helper to convert data coordinates to screen coordinates
    let data_to_screen = |t: f64, r: f64| -> egui::Pos2 {
        let x =
            plot_rect.left() as f64 + width * (t - min_t_scaled) / (max_t_scaled - min_t_scaled);
        let y =
            plot_rect.bottom() as f64 - height * (r - min_r_scaled) / (max_r_scaled - min_r_scaled);
        egui::Pos2 {
            x: x as f32,
            y: y as f32,
        }
    };

    // Draw grid lines (light gray)
    let grid_color = egui::Color32::from_gray(220);
    let t_steps = 5;
    let r_steps = 5;

    for i in 0..=t_steps {
        let t = min_t_scaled + i as f64 / t_steps as f64 * (max_t_scaled - min_t_scaled);
        let p1 = data_to_screen(t, min_r_scaled);
        let p2 = data_to_screen(t, max_r_scaled);
        painter.line_segment([p1, p2], egui::Stroke::new(0.5_f32, grid_color));
    }

    for i in 0..=r_steps {
        let r = min_r_scaled + i as f64 / r_steps as f64 * (max_r_scaled - min_r_scaled);
        let p1 = data_to_screen(min_t_scaled, r);
        let p2 = data_to_screen(max_t_scaled, r);
        painter.line_segment([p1, p2], egui::Stroke::new(0.5_f32, grid_color));
    }

    // Draw segment boundary markers
    for seg in body.profile.iter().take(N - 1) {
        let x = plot_rect.left() as f64
            + width * (seg.end - min_t_scaled) / (max_t_scaled - min_t_scaled);
        let p1 = egui::Pos2::new(x as f32, plot_rect.top());
        let p2 = egui::Pos2::new(x as f32, plot_rect.bottom());
        painter.line_segment(
            [p1, p2],
            egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgba_unmultiplied(100, 100, 200, 120),
            ),
        );
    }

    // Draw profile
    for i in 0..profile_samples.len() - 1 {
        let (t1, r1) = profile_samples[i];
        let (t2, r2) = profile_samples[i + 1];
        let p1 = data_to_screen(t1, r1);
        let p2 = data_to_screen(t2, r2);
        painter.line_segment([p1, p2], egui::Stroke::new(2.0_f32, egui::Color32::RED));
    }

    // Draw axes labels
    let font_id = egui::FontId::proportional(12.0);
    painter.text(
        plot_rect.left_bottom() + egui::Vec2::new(0.0, 15.0),
        egui::Align2::LEFT_TOP,
        &format!("{:.1}", min_t_scaled),
        font_id.clone(),
        egui::Color32::BLACK,
    );
    painter.text(
        plot_rect.right_bottom() + egui::Vec2::new(-40.0, 15.0),
        egui::Align2::LEFT_TOP,
        &format!("{:.1}", max_t_scaled),
        font_id.clone(),
        egui::Color32::BLACK,
    );
    painter.text(
        plot_rect.left_bottom() + egui::Vec2::new(-30.0, -10.0),
        egui::Align2::RIGHT_CENTER,
        &format!("{:.1}", min_r_scaled),
        font_id.clone(),
        egui::Color32::BLACK,
    );
    painter.text(
        plot_rect.left_top() + egui::Vec2::new(-30.0, 5.0),
        egui::Align2::RIGHT_CENTER,
        &format!("{:.1}", max_r_scaled),
        font_id,
        egui::Color32::BLACK,
    );

    Ok(())
}

// ─── 3d mesh preview ───────────────────────────────────────────────────────

fn draw_3d_mesh<const N: usize>(
    ui: &mut egui::Ui,
    body: &RevolutionBody<N>,
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

    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let (sin_yaw, cos_yaw) = yaw.sin_cos();

    let mut projected: Vec<(egui::Pos2, f64)> = Vec::with_capacity(points.len());
    let mut max_extent: f64 = 0.0;

    for p in &points {
        let x0 = p.x;
        let y0 = p.y;
        let z0 = p.z;

        // Rotation inspired by common 3D chart camera controls (yaw + pitch).
        let x1 = x0 * cos_yaw + z0 * sin_yaw;
        let z1 = -x0 * sin_yaw + z0 * cos_yaw;
        let y1 = y0 * cos_pitch - z1 * sin_pitch;
        let z2 = y0 * sin_pitch + z1 * cos_pitch;

        max_extent = max_extent.max(x1.abs()).max(y1.abs()).max(z2.abs());
        projected.push((egui::pos2(x1 as f32, y1 as f32), z2));
    }

    if max_extent <= f64::EPSILON {
        max_extent = 1.0;
    }

    let scale = 0.42_f32 * plot_rect.width().min(plot_rect.height()) / max_extent as f32;
    for (screen_pt, _) in &mut projected {
        screen_pt.x = plot_rect.center().x + screen_pt.x * scale;
        screen_pt.y = plot_rect.center().y - screen_pt.y * scale;
    }

    let mut edge_data: Vec<((usize, usize), f64)> = Vec::new();
    edge_data.reserve(indices.len() * 3);

    for tri in &indices {
        let ai = tri[0] as usize;
        let bi = tri[1] as usize;
        let ci = tri[2] as usize;

        if ai >= projected.len() || bi >= projected.len() || ci >= projected.len() {
            continue;
        }

        let depth = (projected[ai].1 + projected[bi].1 + projected[ci].1) / 3.0;

        let mut ab = (ai, bi);
        if ab.0 > ab.1 {
            ab = (ab.1, ab.0);
        }
        let mut bc = (bi, ci);
        if bc.0 > bc.1 {
            bc = (bc.1, bc.0);
        }
        let mut ca = (ci, ai);
        if ca.0 > ca.1 {
            ca = (ca.1, ca.0);
        }

        edge_data.push((ab, depth));
        edge_data.push((bc, depth));
        edge_data.push((ca, depth));
    }

    edge_data.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    edge_data.dedup_by_key(|entry| entry.0);

    let min_depth = edge_data.first().map(|(_, d)| *d).unwrap_or(0.0);
    let max_depth = edge_data.last().map(|(_, d)| *d).unwrap_or(1.0);
    let depth_span = (max_depth - min_depth).max(1e-9);

    for ((a, b), depth) in edge_data {
        let near = ((depth - min_depth) / depth_span) as f32;
        let line_color = egui::Color32::from_rgb(
            (60.0 + 120.0 * near) as u8,
            (90.0 + 110.0 * near) as u8,
            (130.0 + 90.0 * near) as u8,
        );
        let width = 0.6_f32 + 1.1_f32 * near;
        painter.line_segment(
            [projected[a].0, projected[b].0],
            egui::Stroke::new(width, line_color),
        );
    }
}

// ─── eframe app ──────────────────────────────────────────────────────────────

impl eframe::App for RevolutionBodyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("controls")
            .min_width(300.0)
            .max_width(400.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_controls(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Ok(body) = self.try_build_body() {
                // 2D profile preview
                ui.heading("2D Profile Preview");
                ui.group(|ui| {
                    ui.set_min_size(egui::Vec2::new(400.0, 300.0));
                    if let Err(e) = draw_2d_profile(ui, &body) {
                        ui.colored_label(Color32::LIGHT_RED, format!("2D Render error: {}", e));
                    }
                });

                ui.separator();

                // 3D mesh preview
                ui.heading("3D Mesh Preview");
                ui.label("Wireframe render with camera yaw/pitch");
                draw_3d_mesh(
                    ui,
                    &body,
                    self.camera_pitch,
                    self.camera_yaw,
                    self.resolution,
                );
                let (stats_vertices, stats_triangles) = self.get_mesh_stats().unwrap_or((0, 0));
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

        ctx.request_repaint();
    }
}
