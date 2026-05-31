use crate::body::Segment;
use egui::{Color32, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use mint::Point2;

const SEGMENT_COLORS: [Color32; 3] = [
    Color32::from_rgb(80, 200, 130),
    Color32::from_rgb(102, 178, 255),
    Color32::from_rgb(245, 203, 92),
];

pub fn draw_segment_chart(ui: &mut Ui, segments: Option<&[Segment]>, error: Option<&str>) {
    let desired = ui.available_size();
    draw_segment_chart_with_size(ui, desired, segments, error);
}

pub fn draw_segment_chart_sized(
    ui: &mut Ui,
    height: f32,
    segments: Option<&[Segment]>,
    error: Option<&str>,
) {
    let desired = egui::vec2(ui.available_width(), height);
    draw_segment_chart_with_size(ui, desired, segments, error);
}

fn draw_segment_chart_with_size(
    ui: &mut Ui,
    desired: Vec2,
    segments: Option<&[Segment]>,
    error: Option<&str>,
) {
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

    let draw_rect = rect.shrink2(Vec2::new(16.0, 32.0));

    let map_pt = |x: f64, y: f64| -> Pos2 {
        let nx = ((x - min_x) / x_span) as f32;
        let ny = 1.0 - ((y - min_y) / y_span) as f32;
        Pos2::new(
            draw_rect.left() + nx * draw_rect.width(),
            draw_rect.top() + ny * draw_rect.height(),
        )
    };

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
                sp + Vec2::new(7.0, -14.0),
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

pub fn draw_xy_line_chart(
    ui: &mut Ui,
    title: &str,
    points: &[(f64, f64)],
    x_name: &str,
    y_name: &str,
) {
    draw_xy_multi_line_chart_sized(
        ui,
        title,
        155.0,
        &[("line", points, Color32::from_rgb(116, 192, 252))],
        x_name,
        y_name,
    );
}

pub fn draw_xy_multi_line_chart_sized(
    ui: &mut Ui,
    title: &str,
    height: f32,
    series: &[(&str, &[(f64, f64)], Color32)],
    x_name: &str,
    y_name: &str,
) {
    ui.label(title);
    let (resp, painter) =
        ui.allocate_painter(egui::vec2(ui.available_width(), height), Sense::hover());
    let rect = resp.rect;
    painter.rect_filled(rect, 4.0, Color32::from_rgb(20, 25, 32));

    let valid_series = series
        .iter()
        .filter(|(_, points, _)| points.len() >= 2)
        .collect::<Vec<_>>();

    if valid_series.is_empty() {
        return;
    }

    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (_, points, _) in &valid_series {
        for (x, y) in *points {
            x_min = x_min.min(*x);
            x_max = x_max.max(*x);
            y_min = y_min.min(*y);
            y_max = y_max.max(*y);
        }
    }

    let x_span = (x_max - x_min).max(1e-9);
    let y_span = (y_max - y_min).max(1e-9);
    let inner = rect.shrink2(Vec2::new(12.0, 10.0));

    let to_pos = |x: f64, y: f64| -> Pos2 {
        let u = ((x - x_min) / x_span) as f32;
        let v = ((y - y_min) / y_span) as f32;
        Pos2::new(
            inner.left() + inner.width() * u,
            inner.bottom() - inner.height() * v,
        )
    };

    for (_, points, color) in &valid_series {
        let path = points
            .iter()
            .map(|(x, y)| to_pos(*x, *y))
            .collect::<Vec<_>>();
        painter.add(Shape::line(path, Stroke::new(2.0_f32, *color)));
    }

    if valid_series.len() > 1 {
        let mut x_off = 0.0;
        for (name, _, color) in &valid_series {
            painter.text(
                inner.left_top() + egui::vec2(x_off, 14.0),
                egui::Align2::LEFT_TOP,
                *name,
                egui::TextStyle::Small.resolve(ui.style()),
                *color,
            );
            x_off += 64.0;
        }
    }

    painter.text(
        inner.left_top(),
        egui::Align2::LEFT_TOP,
        format!("{} -> {}", x_name, y_name),
        egui::TextStyle::Small.resolve(ui.style()),
        Color32::from_gray(145),
    );
}

pub fn show_frequency_peaks(ui: &mut Ui, peaks: &[(u32, f32)], empty_label: &str) {
    if peaks.is_empty() {
        ui.small(empty_label);
        return;
    }

    for (frequency, amplitude) in peaks {
        ui.monospace(format!("{frequency:>5} Hz  {amplitude:.6e}"));
    }
}

pub struct MeshProjector {
    center: Pos2,
    scale: f32,
    pitch: f64,
    yaw: f64,
    max_extent: f64,
}

impl MeshProjector {
    pub fn from_xyz_points(
        points: &[(f64, f64, f64)],
        plot_rect: Rect,
        pitch: f64,
        yaw: f64,
        scale_ratio: f32,
    ) -> (Self, Vec<(Pos2, f64)>) {
        let mut rotated = Vec::with_capacity(points.len());
        let mut max_extent: f64 = 0.0;
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();

        for (x, y, z) in points {
            let x1 = *x * cos_yaw + *z * sin_yaw;
            let z1 = -*x * sin_yaw + *z * cos_yaw;
            let y1 = *y * cos_pitch - z1 * sin_pitch;
            let z2 = *y * sin_pitch + z1 * cos_pitch;
            max_extent = max_extent.max(x1.abs()).max(y1.abs()).max(z2.abs());
            rotated.push((x1, y1, z2));
        }

        if max_extent <= f64::EPSILON {
            max_extent = 1.0;
        }

        let scale = scale_ratio * plot_rect.width().min(plot_rect.height()) / max_extent as f32;
        let center = plot_rect.center();

        let projected = rotated
            .iter()
            .map(|(x, y, z)| {
                (
                    Pos2::new(center.x + *x as f32 * scale, center.y - *y as f32 * scale),
                    *z,
                )
            })
            .collect::<Vec<_>>();

        (
            Self {
                center,
                scale,
                pitch,
                yaw,
                max_extent,
            },
            projected,
        )
    }

    pub fn max_extent(&self) -> f64 {
        self.max_extent
    }

    pub fn project_xyz(&self, x: f64, y: f64, z: f64) -> (Pos2, f64) {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

        let x1 = x * cos_yaw + z * sin_yaw;
        let z1 = -x * sin_yaw + z * cos_yaw;
        let y1 = y * cos_pitch - z1 * sin_pitch;
        let z2 = y * sin_pitch + z1 * cos_pitch;

        (
            Pos2::new(
                self.center.x + x1 as f32 * self.scale,
                self.center.y - y1 as f32 * self.scale,
            ),
            z2,
        )
    }
}

pub fn draw_depth_wireframe(painter: &Painter, projected: &[(Pos2, f64)], indices: &[[u32; 3]]) {
    let mut edge_data: Vec<((usize, usize), f64)> = Vec::new();
    edge_data.reserve(indices.len() * 3);

    for tri in indices {
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
        let line_color = Color32::from_rgb(
            (60.0 + 120.0 * near) as u8,
            (90.0 + 110.0 * near) as u8,
            (130.0 + 90.0 * near) as u8,
        );
        let width = 0.6_f32 + 1.1_f32 * near;
        painter.line_segment(
            [projected[a].0, projected[b].0],
            Stroke::new(width, line_color),
        );
    }
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
