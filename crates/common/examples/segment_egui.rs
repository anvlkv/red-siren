use common::body::Segment;
use common::egui_helpers::{
    draw_segment_chart, run_native_app, show_scrolled_left_panel_inside, CurveKind,
    DirectSegmentConfig,
};
use eframe::egui::{self, Color32};

fn main() -> eframe::Result<()> {
    run_native_app("Segment preview", [1100.0, 720.0], None, |_cc| {
        SegmentApp::default()
    })
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
        self.first.show_controls(ui, "Segment 1", "segment_1_kind");

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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let preview = self.preview_result();

        show_scrolled_left_panel_inside(ui, "controls", 300.0, None, |ui| {
            self.show_controls(ui, &preview);
        });

        egui::CentralPanel::default().show_inside(ui, move |ui| {
            let segments = preview.as_ref().ok().map(|segments| segments.as_slice());
            let error = preview.as_ref().err().map(|err| err.as_str());
            draw_segment_chart(ui, segments, error);
        });
        ui.ctx().request_repaint();
    }
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
