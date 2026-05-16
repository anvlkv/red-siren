use crate::body::Segment;
use egui::{Color32, ComboBox, DragValue, Slider, Ui};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CurveKind {
    Line,
    Constant,
    Parabolic,
    Hyperbolic,
    ArcSweep,
}

impl CurveKind {
    pub fn label(self) -> &'static str {
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
pub struct DirectSegmentConfig {
    pub start: f64,
    pub end: f64,
    pub density: f64,
    pub kind: CurveKind,
    pub line_m: f64,
    pub line_b: f64,
    pub const_value: f64,
    pub para_a: f64,
    pub para_b: f64,
    pub para_c: f64,
    pub hyp_a: f64,
    pub hyp_b: f64,
    pub hyp_c: f64,
    pub arc_center_y: f64,
    pub arc_radius: f64,
    pub arc_start_angle: f64,
    pub arc_sweep_angle: f64,
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
    pub fn build_segment(&self) -> Result<Segment, String> {
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

    pub fn show_controls(
        &mut self,
        ui: &mut Ui,
        heading: &str,
        kind_id_salt: impl std::hash::Hash,
    ) {
        ui.heading(heading);
        ui.label("Domain");
        ui.add(Slider::new(&mut self.start, -10.0..=10.0).text("start"));
        ui.add(Slider::new(&mut self.end, -10.0..=10.0).text("end"));
        ui.add(Slider::new(&mut self.density, 1.0..=64.0).text("sampling density"));

        ui.separator();
        ui.label("Curve type");
        ComboBox::from_id_salt(kind_id_salt)
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
        ui.label("Parameters");
        match self.kind {
            CurveKind::Line => {
                ui.add(Slider::new(&mut self.line_m, -10.0..=10.0).text("m (slope)"));
                ui.add(Slider::new(&mut self.line_b, -10.0..=10.0).text("b (intercept)"));
            }
            CurveKind::Constant => {
                ui.add(Slider::new(&mut self.const_value, -10.0..=10.0).text("value"));
            }
            CurveKind::Parabolic => {
                ui.add(Slider::new(&mut self.para_a, -5.0..=5.0).text("a"));
                ui.add(Slider::new(&mut self.para_b, -10.0..=10.0).text("b"));
                ui.add(Slider::new(&mut self.para_c, -10.0..=10.0).text("c"));
            }
            CurveKind::Hyperbolic => {
                ui.add(Slider::new(&mut self.hyp_a, -5.0..=5.0).text("a"));
                ui.add(
                    DragValue::new(&mut self.hyp_b)
                        .speed(0.05)
                        .prefix("b (pole) "),
                );
                ui.add(Slider::new(&mut self.hyp_c, -10.0..=10.0).text("c"));
                if self.hyp_b > self.start && self.hyp_b < self.end {
                    ui.colored_label(
                        Color32::from_rgb(220, 180, 60),
                        "b (pole) must be outside [start, end]",
                    );
                }
            }
            CurveKind::ArcSweep => {
                ui.add(Slider::new(&mut self.arc_center_y, -10.0..=10.0).text("center_y"));
                ui.add(Slider::new(&mut self.arc_radius, 0.05..=10.0).text("radius"));
                ui.add(
                    Slider::new(
                        &mut self.arc_start_angle,
                        -std::f64::consts::PI..=std::f64::consts::PI,
                    )
                    .text("start_angle (rad)"),
                );
                ui.add(
                    Slider::new(
                        &mut self.arc_sweep_angle,
                        -std::f64::consts::TAU..=std::f64::consts::TAU,
                    )
                    .text("sweep_angle (rad)"),
                );
            }
        }
    }
}
