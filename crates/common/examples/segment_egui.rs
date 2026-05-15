use common::geometry::Segment;
use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke};
use mint::Point2;

const SEGMENT_COLORS: [Color32; 3] = [
    Color32::from_rgb(80, 200, 130),
    Color32::from_rgb(102, 178, 255),
    Color32::from_rgb(245, 203, 92),
];

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Segment preview")
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Segment preview",
        options,
        Box::new(|_cc| Ok(Box::new(SegmentApp::default()))),
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum PreviewMode {
    Single,
    Joined,
}

impl PreviewMode {
    fn label(self) -> &'static str {
        match self {
            Self::Single => "Single segment",
            Self::Joined => "3-segment join",
        }
    }
}

// ─── app state ───────────────────────────────────────────────────────────────

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
        egui::ComboBox::from_label("kind")
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

struct ContinuationConfig {
    kind: CurveKind,
    to_t: f64,
    density: f64,
    to_value: f64,
    pole_t: f64,
    arc_radius: f64,
    arc_sweep_angle: f64,
    arc_concave_up: bool,
}

impl Default for ContinuationConfig {
    fn default() -> Self {
        Self {
            kind: CurveKind::Parabolic,
            to_t: 7.0,
            density: 16.0,
            to_value: -0.5,
            pole_t: 9.0,
            arc_radius: 2.0,
            arc_sweep_angle: std::f64::consts::FRAC_PI_2,
            arc_concave_up: false,
        }
    }
}

impl ContinuationConfig {
    fn build_segment(&self, prev: &Segment) -> Result<Segment, String> {
        match self.kind {
            CurveKind::Line => Segment::continue_line(prev, self.to_t, self.density),
            CurveKind::Constant => Segment::continue_constant(prev, self.to_t, self.density),
            CurveKind::Parabolic => {
                Segment::continue_parabolic(prev, self.to_t, self.to_value, self.density)
            }
            CurveKind::Hyperbolic => {
                Segment::continue_hyperbolic(prev, self.to_t, self.pole_t, self.density)
            }
            CurveKind::ArcSweep => Segment::continue_arc_sweep(
                prev,
                self.to_t,
                self.arc_radius,
                self.arc_sweep_angle,
                self.arc_concave_up,
                self.density,
            ),
        }
        .map_err(|err| err.to_string())
    }

    fn show_controls(&mut self, ui: &mut egui::Ui, heading: &str, start_t: Option<f64>) {
        ui.heading(heading);
        if let Some(start_t) = start_t {
            ui.small(format!("start is fixed at previous end: {start_t:.3}"));
        } else {
            ui.small("start becomes available after segment 1 resolves");
        }

        ui.add(egui::Slider::new(&mut self.to_t, -10.0..=20.0).text("to_t"));
        ui.add(egui::Slider::new(&mut self.density, 1.0..=64.0).text("sampling density"));

        ui.separator();
        ui.label("Continuation type");
        egui::ComboBox::from_label(format!("{} kind", heading))
            .selected_text(self.kind.label())
            .show_ui(ui, |ui| {
                for kind in [
                    CurveKind::Line,
                    CurveKind::Constant,
                    CurveKind::Parabolic,
                    CurveKind::Hyperbolic,
                    CurveKind::ArcSweep,
                ] {
                    ui.selectable_value(&mut self.kind, kind, kind.label());
                }
            });

        ui.separator();
        ui.label("Available parameters");
        match self.kind {
            CurveKind::Line | CurveKind::Constant => {}
            CurveKind::Parabolic => {
                ui.add(egui::Slider::new(&mut self.to_value, -10.0..=10.0).text("to_value"));
            }
            CurveKind::Hyperbolic => {
                ui.add(
                    egui::DragValue::new(&mut self.pole_t)
                        .speed(0.05)
                        .prefix("pole_t "),
                );
                ui.small("pole_t must stay outside the new segment interval");
            }
            CurveKind::ArcSweep => {
                ui.add(egui::Slider::new(&mut self.arc_radius, 0.05..=10.0).text("radius"));
                ui.add(
                    egui::Slider::new(
                        &mut self.arc_sweep_angle,
                        -std::f64::consts::TAU..=std::f64::consts::TAU,
                    )
                    .text("sweep_angle (rad)"),
                );
                ui.checkbox(&mut self.arc_concave_up, "concave_up");
            }
        }
    }
}

struct SegmentApp {
    preview_mode: PreviewMode,
    first: DirectSegmentConfig,
    second: ContinuationConfig,
    third: ContinuationConfig,
}

impl Default for SegmentApp {
    fn default() -> Self {
        Self {
            preview_mode: PreviewMode::Single,
            first: DirectSegmentConfig::default(),
            second: ContinuationConfig::default(),
            third: ContinuationConfig {
                kind: CurveKind::Line,
                to_t: 10.0,
                ..ContinuationConfig::default()
            },
        }
    }
}

impl SegmentApp {
    fn preview_result(&self) -> Result<Vec<Segment>, String> {
        let first = self
            .first
            .build_segment()
            .map_err(|err| format!("segment 1: {err}"))?;

        match self.preview_mode {
            PreviewMode::Single => Ok(vec![first]),
            PreviewMode::Joined => {
                let second = self
                    .second
                    .build_segment(&first)
                    .map_err(|err| format!("segment 2: {err}"))?;
                let third = self
                    .third
                    .build_segment(&second)
                    .map_err(|err| format!("segment 3: {err}"))?;
                Ok(vec![first, second, third])
            }
        }
    }

    fn show_controls(&mut self, ui: &mut egui::Ui, preview: &Result<Vec<Segment>, String>) {
        ui.horizontal(|ui| {
            for mode in [PreviewMode::Single, PreviewMode::Joined] {
                ui.selectable_value(&mut self.preview_mode, mode, mode.label());
            }
        });

        ui.separator();
        self.first.show_controls(ui, "Segment 1");

        if self.preview_mode == PreviewMode::Joined {
            ui.separator();
            let first_preview = self.first.build_segment().ok();
            self.second
                .show_controls(ui, "Segment 2", first_preview.as_ref().map(|seg| seg.end));

            ui.separator();
            let second_preview = first_preview
                .as_ref()
                .and_then(|seg| self.second.build_segment(seg).ok());
            self.third
                .show_controls(ui, "Segment 3", second_preview.as_ref().map(|seg| seg.end));
        }

        if let Err(err) = preview {
            ui.separator();
            ui.colored_label(Color32::LIGHT_RED, format!("Preview error: {err}"));
        }

        if let Ok(segments) = preview {
            ui.separator();
            ui.label("Resolved segments");
            for (index, segment) in segments.iter().enumerate() {
                show_segment_metrics(ui, segment, &format!("Segment {}", index + 1));
                if index + 1 < segments.len() {
                    ui.separator();
                }
            }
        }
    }
}

// ─── eframe app ──────────────────────────────────────────────────────────────

impl eframe::App for SegmentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let preview = self.preview_result();

        egui::SidePanel::left("controls")
            .min_width(300.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_controls(ui, &preview);
                });
            });

        egui::CentralPanel::default().show(ctx, move |ui| {
            let segments = preview.as_ref().ok().map(|segments| segments.as_slice());
            let error = preview.as_ref().err().map(|err| err.as_str());
            draw_chart(ui, segments, error);
        });

        ctx.request_repaint();
    }
}

// ─── chart drawing ───────────────────────────────────────────────────────────

fn draw_chart(ui: &mut egui::Ui, segments: Option<&[Segment]>, error: Option<&str>) {
    let desired = ui.available_size();
    let (response, painter) = ui.allocate_painter(desired, Sense::hover());
    let rect = response.rect;

    painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 20, 26));

    let Some(segments) = segments else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            error.unwrap_or("No valid segment"),
            egui::TextStyle::Body.resolve(ui.style()),
            Color32::LIGHT_RED,
        );
        return;
    };

    if segments.is_empty() {
        return;
    }

    let sampled_sets: Vec<Vec<Point2<f64>>> = segments.iter().map(sample_segment_points).collect();

    if sampled_sets.iter().all(|points| points.len() < 2) {
        return;
    }

    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for points in &sampled_sets {
        for point in points {
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }
    }

    let y_margin = ((max_y - min_y) * 0.22).max(0.5);
    min_y -= y_margin;
    max_y += y_margin;

    let first = &segments[0];
    let last = &segments[segments.len() - 1];
    let total_span = (last.end - first.start).max(1e-9);
    let ext_span = (total_span * 0.16).max(0.5);
    let min_x = first.start - ext_span;
    let max_x = last.end + ext_span;
    let x_span = (max_x - min_x).max(1e-9);
    let y_span = (max_y - min_y).max(1e-9);

    let draw_rect = rect.shrink2(egui::Vec2::new(16.0, 32.0));

    // Mapping closure (captures only Copy types).
    let map_pt = |x: f64, y: f64| -> Pos2 {
        let nx = ((x - min_x) / x_span) as f32;
        let ny = 1.0 - ((y - min_y) / y_span) as f32;
        Pos2::new(
            draw_rect.left() + nx * draw_rect.width(),
            draw_rect.top() + ny * draw_rect.height(),
        )
    };

    // ── y = 0 axis ───────────────────────────────────────────────────────────
    if min_y < 0.0 && max_y > 0.0 {
        let y0 = map_pt(0.0, 0.0).y;
        painter.line_segment(
            [
                Pos2::new(draw_rect.left(), y0),
                Pos2::new(draw_rect.right(), y0),
            ],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(150, 150, 170, 55)),
        );
    }

    let boundary = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(100, 160, 255, 75));
    for boundary_t in segments
        .iter()
        .map(|segment| segment.start)
        .chain(std::iter::once(last.end))
    {
        let x = map_pt(boundary_t, 0.0).x;
        painter.line_segment(
            [
                Pos2::new(x, draw_rect.top()),
                Pos2::new(x, draw_rect.bottom()),
            ],
            boundary,
        );
    }

    let ext_n = 40usize;
    let dim = Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(120, 130, 150, 70));

    let mut ext_l: Vec<Pos2> = Vec::with_capacity(ext_n + 1);
    for i in 0..=ext_n {
        let t = min_x + ext_span * (i as f64 / ext_n as f64);
        let y = first.evaluate(t);
        if y.is_finite() && y >= min_y && y <= max_y {
            ext_l.push(map_pt(t, y));
        } else if ext_l.len() >= 2 {
            painter.add(Shape::line(std::mem::take(&mut ext_l), dim));
        } else {
            ext_l.clear();
        }
    }
    if ext_l.len() >= 2 {
        painter.add(Shape::line(ext_l, dim));
    }

    let mut ext_r: Vec<Pos2> = Vec::with_capacity(ext_n + 1);
    for i in 0..=ext_n {
        let t = last.end + ext_span * (i as f64 / ext_n as f64);
        let y = last.evaluate(t);
        if y.is_finite() && y >= min_y && y <= max_y {
            ext_r.push(map_pt(t, y));
        } else if ext_r.len() >= 2 {
            painter.add(Shape::line(std::mem::take(&mut ext_r), dim));
        } else {
            ext_r.clear();
        }
    }
    if ext_r.len() >= 2 {
        painter.add(Shape::line(ext_r, dim));
    }

    for (index, points) in sampled_sets.iter().enumerate() {
        if points.len() < 2 {
            continue;
        }

        let color = SEGMENT_COLORS[index % SEGMENT_COLORS.len()];
        let screen_points: Vec<Pos2> = points
            .iter()
            .map(|point| map_pt(point.x, point.y))
            .collect();
        painter.add(Shape::line(screen_points, Stroke::new(2.5_f32, color)));

        for point in points {
            painter.circle_filled(
                map_pt(point.x, point.y),
                2.2,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 150),
            );
        }
    }

    let font = egui::TextStyle::Small.resolve(ui.style());
    for (pt, label) in [
        (
            sampled_sets.first().and_then(|points| points.first()),
            format!("t={:.2}  y={:.3}", first.start, first.start_value()),
        ),
        (
            sampled_sets.last().and_then(|points| points.last()),
            format!("t={:.2}  y={:.3}", last.end, last.end_value()),
        ),
    ] {
        if let Some(p) = pt {
            let sp = map_pt(p.x, p.y);
            painter.circle_filled(sp, 5.0, Color32::from_rgb(245, 203, 92));
            painter.text(
                sp + egui::Vec2::new(7.0, -14.0),
                egui::Align2::LEFT_TOP,
                &label,
                font.clone(),
                Color32::from_rgb(245, 203, 92),
            );
        }
    }

    for segment in segments.iter().take(segments.len().saturating_sub(1)) {
        let join = map_pt(segment.end, segment.end_value());
        painter.circle_filled(join, 4.0, Color32::WHITE);
    }

    let tick_font = egui::TextStyle::Small.resolve(ui.style());
    for (index, tx) in segments
        .iter()
        .map(|segment| segment.start)
        .chain(std::iter::once(last.end))
        .enumerate()
    {
        let label = if index == 0 {
            format!("start {:.2}", tx)
        } else if index == segments.len() {
            format!("end {:.2}", tx)
        } else {
            format!("join {:.2}", tx)
        };
        let sx = map_pt(tx, 0.0).x;
        if sx >= draw_rect.left() && sx <= draw_rect.right() {
            painter.text(
                Pos2::new(sx, draw_rect.bottom() + 4.0),
                egui::Align2::CENTER_TOP,
                &label,
                tick_font.clone(),
                Color32::from_gray(150),
            );
        }
    }

    painter.text(
        Pos2::new(draw_rect.left(), draw_rect.top() - 18.0),
        egui::Align2::LEFT_TOP,
        legend_text(segments.len()),
        tick_font,
        Color32::from_gray(170),
    );
}

fn sample_segment_points(segment: &Segment) -> Vec<Point2<f64>> {
    let samples = ((segment.end - segment.start) * segment.sampling_density)
        .ceil()
        .max(1.0) as usize;

    let mut points = Vec::with_capacity(samples + 1);
    for index in 0..=samples {
        let u = index as f64 / samples as f64;
        let t = segment.start + (segment.end - segment.start) * u;
        let y = segment.evaluate(t);
        if t.is_finite() && y.is_finite() {
            points.push(Point2 { x: t, y });
        }
    }

    points
}

fn show_segment_metrics(ui: &mut egui::Ui, segment: &Segment, heading: &str) {
    ui.label(egui::RichText::new(heading).strong());
    egui::Grid::new(heading)
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            ui.label("");
            ui.label(egui::RichText::new("start").strong());
            ui.label(egui::RichText::new("end").strong());
            ui.end_row();

            ui.label("t");
            ui.monospace(format!("{:.4}", segment.start));
            ui.monospace(format!("{:.4}", segment.end));
            ui.end_row();

            ui.label("y");
            ui.monospace(format!("{:.4}", segment.start_value()));
            ui.monospace(format!("{:.4}", segment.end_value()));
            ui.end_row();

            ui.label("dy/dt");
            ui.monospace(format!("{:.4}", segment.start_slope()));
            ui.monospace(format!("{:.4}", segment.end_slope()));
            ui.end_row();

            ui.label("d2y/dt2");
            ui.monospace(format!("{:.4}", segment.start_curvature()));
            ui.monospace(format!("{:.4}", segment.end_curvature()));
            ui.end_row();
        });
}

fn legend_text(segment_count: usize) -> String {
    let labels = ["S1", "S2", "S3"];
    labels
        .iter()
        .take(segment_count)
        .enumerate()
        .map(|(index, label)| format!("{label} {}", labels_color_name(index)))
        .collect::<Vec<_>>()
        .join("   ")
}

fn labels_color_name(index: usize) -> &'static str {
    match index {
        0 => "green",
        1 => "blue",
        _ => "gold",
    }
}
