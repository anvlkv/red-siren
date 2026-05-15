use common::geometry::{
    thickness_map_from_axial_samples, Embodied, EmbodiedPoint3, EmbodiedTriangle, RevolutionAxis,
    RevolutionBody, Segment, ThickBody,
};
use eframe::egui::{self, Color32, Shape, Stroke};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Thick RevolutionBody - Surface Mesh Viewer")
            .with_inner_size([1450.0, 920.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Thick RevolutionBody - Surface Mesh Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(ThickRevolutionBodyApp::default()))),
    )
}

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

struct ThickRevolutionBodyApp {
    profile_config_1: DirectSegmentConfig,
    profile_config_2: DirectSegmentConfig,
    face_thickness_config: DirectSegmentConfig,
    backface_thickness_config: DirectSegmentConfig,
    axis: RevolutionAxis,
    resolution: usize,
    validation_error: Option<String>,
    camera_pitch: f64,
    camera_yaw: f64,
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
            camera_pitch: 0.5,
            camera_yaw: 0.7,
        }
    }
}

impl ThickRevolutionBodyApp {
    fn try_build_scene(&self) -> Result<(RevolutionBody<2>, ThickBody<RevolutionBody<2>>), String> {
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

        let body = RevolutionBody::new([seg1, seg2], self.axis)
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
        let thickness_map =
            thickness_map_from_axial_samples(&axial_positions, self.resolution, |u| {
                let face_t = face_segment.start + u * face_span;
                let backface_t = backface_segment.start + u * back_span;
                let face = face_segment.evaluate(face_t).max(0.0);
                let backface = backface_segment.evaluate(backface_t).max(0.0);
                (face, backface)
            })
            .ok_or_else(|| "thickness map construction failed (invalid values)".to_string())?;

        Ok((body.clone(), ThickBody::new(body, thickness_map)))
    }

    fn get_mesh_stats(&self) -> Option<(usize, usize)> {
        match self.try_build_scene() {
            Ok((_body, thick)) => Some((
                thick.sample_points(self.resolution).len(),
                thick.mesh_indices(self.resolution).len(),
            )),
            Err(_) => None,
        }
    }

    fn sample_thickness_curves(
        &self,
        body: &RevolutionBody<2>,
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

    fn show_controls(&mut self, ui: &mut egui::Ui) {
        self.profile_config_1.show_controls(ui, "Profile Segment 1");

        ui.separator();
        self.profile_config_2.show_controls(ui, "Profile Segment 2");

        ui.separator();
        self.face_thickness_config
            .show_controls(ui, "Face Thickness Segment");

        ui.separator();
        self.backface_thickness_config
            .show_controls(ui, "Backface Thickness Segment");

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

        match self.try_build_scene() {
            Ok(_) => {
                ui.colored_label(Color32::GREEN, "✓ Valid configuration");
                self.validation_error = None;

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

fn draw_2d_profile<const N: usize>(ui: &mut egui::Ui, body: &RevolutionBody<N>) {
    let profile_samples = body.sampled_profile_points(64);
    if profile_samples.len() < 2 {
        return;
    }

    let (min_t, max_t) = body.height_range();
    let max_r = profile_samples
        .iter()
        .map(|(_, r)| *r)
        .fold(0.0f64, f64::max)
        .max(0.5);

    let desired_size = egui::Vec2::new(420.0, 300.0);
    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());

    let rect = response.rect;
    let plot_rect = rect.shrink(10.0);

    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    painter.rect_stroke(
        plot_rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
        egui::StrokeKind::Middle,
    );

    let margin_r = (max_r * 0.15).max(0.2);
    let margin_t = ((max_t - min_t) * 0.1).max(0.4);
    let min_t_scaled = min_t - margin_t;
    let max_t_scaled = max_t + margin_t;
    let min_r_scaled = 0.0;
    let max_r_scaled = max_r + margin_r;

    let width = plot_rect.width() as f64;
    let height = plot_rect.height() as f64;

    let data_to_screen = |t: f64, r: f64| -> egui::Pos2 {
        let x =
            plot_rect.left() as f64 + width * (t - min_t_scaled) / (max_t_scaled - min_t_scaled);
        let y =
            plot_rect.bottom() as f64 - height * (r - min_r_scaled) / (max_r_scaled - min_r_scaled);
        egui::Pos2::new(x as f32, y as f32)
    };

    for i in 0..profile_samples.len() - 1 {
        let (t1, r1) = profile_samples[i];
        let (t2, r2) = profile_samples[i + 1];
        painter.line_segment(
            [data_to_screen(t1, r1), data_to_screen(t2, r2)],
            egui::Stroke::new(2.0_f32, Color32::RED),
        );
    }
}

fn draw_2d_thickness_preview(ui: &mut egui::Ui, samples: &[(f64, f64, f64)]) {
    if samples.len() < 2 {
        return;
    }

    let min_t = samples.first().map(|s| s.0).unwrap_or(0.0);
    let max_t = samples.last().map(|s| s.0).unwrap_or(1.0);
    let max_thickness = samples
        .iter()
        .map(|(_, face, back)| face.max(*back))
        .fold(0.0_f64, f64::max)
        .max(0.02);

    let desired_size = egui::Vec2::new(420.0, 220.0);
    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());

    let rect = response.rect;
    let plot_rect = rect.shrink(10.0);

    painter.rect_filled(rect, 0.0, Color32::WHITE);
    painter.rect_stroke(
        plot_rect,
        0.0,
        egui::Stroke::new(1.0_f32, Color32::BLACK),
        egui::StrokeKind::Middle,
    );

    let margin_t = ((max_t - min_t) * 0.08).max(0.1);
    let min_t_scaled = min_t - margin_t;
    let max_t_scaled = max_t + margin_t;
    let max_thickness_scaled = (max_thickness * 1.2).max(0.05);

    let width = plot_rect.width() as f64;
    let height = plot_rect.height() as f64;

    let data_to_screen = |t: f64, y: f64| -> egui::Pos2 {
        let x =
            plot_rect.left() as f64 + width * (t - min_t_scaled) / (max_t_scaled - min_t_scaled);
        let sy = plot_rect.bottom() as f64 - height * y / max_thickness_scaled;
        egui::Pos2::new(x as f32, sy as f32)
    };

    let face_color = Color32::from_rgb(216, 69, 69);
    let back_color = Color32::from_rgb(55, 121, 214);

    for i in 0..samples.len() - 1 {
        let (t1, f1, b1) = samples[i];
        let (t2, f2, b2) = samples[i + 1];

        painter.line_segment(
            [data_to_screen(t1, f1), data_to_screen(t2, f2)],
            egui::Stroke::new(2.0_f32, face_color),
        );
        painter.line_segment(
            [data_to_screen(t1, b1), data_to_screen(t2, b2)],
            egui::Stroke::new(2.0_f32, back_color),
        );
    }

    let font_id = egui::FontId::proportional(11.0);
    painter.text(
        plot_rect.left_top() + egui::vec2(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        "Face",
        font_id.clone(),
        face_color,
    );
    painter.text(
        plot_rect.left_top() + egui::vec2(52.0, 6.0),
        egui::Align2::LEFT_TOP,
        "Backface",
        font_id,
        back_color,
    );
}

fn draw_3d_mesh<M>(ui: &mut egui::Ui, mesh: &M, pitch: f64, yaw: f64, resolution: usize)
where
    M: Embodied<Vertex = EmbodiedPoint3, Index = EmbodiedTriangle>,
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

    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let (sin_yaw, cos_yaw) = yaw.sin_cos();

    let mut projected: Vec<(egui::Pos2, f64)> = Vec::with_capacity(points.len());
    let mut max_extent: f64 = 0.0;

    for p in &points {
        let x1 = p.x * cos_yaw + p.z * sin_yaw;
        let z1 = -p.x * sin_yaw + p.z * cos_yaw;
        let y1 = p.y * cos_pitch - z1 * sin_pitch;
        let z2 = p.y * sin_pitch + z1 * cos_pitch;

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

    let mut face_data: Vec<([usize; 3], f64, Color32)> = Vec::with_capacity(indices.len());
    let mut edge_data: Vec<((usize, usize), f64)> = Vec::new();
    edge_data.reserve(indices.len() * 3);

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

impl eframe::App for ThickRevolutionBodyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("controls")
            .min_width(320.0)
            .max_width(430.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_controls(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.try_build_scene() {
            Ok((body, thick_body)) => {
                ui.columns(2, |cols| {
                    cols[0].heading("2D Profile Preview (2 profile segments)");
                    cols[0].group(|ui| {
                        ui.set_min_size(egui::Vec2::new(420.0, 300.0));
                        draw_2d_profile(ui, &body);
                    });

                    cols[1].heading("2D Thickness Preview");
                    cols[1].group(|ui| match self.sample_thickness_curves(&body, 96) {
                        Ok(samples) => {
                            ui.set_min_size(egui::Vec2::new(420.0, 300.0));
                            draw_2d_thickness_preview(ui, &samples);
                        }
                        Err(err) => {
                            ui.colored_label(
                                Color32::LIGHT_RED,
                                format!("Thickness preview error: {}", err),
                            );
                        }
                    });
                });

                ui.separator();
                ui.heading("3D Thick Mesh Preview");
                ui.label(
                    "Filled render: outside (warm), inside (cool), rim (green) + wireframe overlay",
                );
                draw_3d_mesh(
                    ui,
                    &thick_body,
                    self.camera_pitch,
                    self.camera_yaw,
                    self.resolution,
                );

                if let Some((stats_vertices, stats_triangles)) = self.get_mesh_stats() {
                    ui.label(format!(
                        "Mesh: {} vertices, {} triangles",
                        stats_vertices, stats_triangles
                    ));
                }
            }
            Err(_) => {
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
