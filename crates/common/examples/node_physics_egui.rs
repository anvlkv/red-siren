use common::config::{
    BaseControls, BowControls, FringeControls, MaterialControls, NodeControls, NodePhysics,
    RimControls, ShoulderControls,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke};
use mint::Point2;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NodePhysics egui preview")
            .with_inner_size([1200.0, 760.0]),
        ..Default::default()
    };

    eframe::run_native(
        "NodePhysics egui preview",
        options,
        Box::new(|_cc| Ok(Box::new(NodePhysicsApp::default()))),
    )
}

#[derive(Clone, Copy)]
struct Preset {
    name: &'static str,
    node: NodePhysics,
}

struct NodePhysicsApp {
    node: NodePhysics,
    samples_per_side: usize,
    presets: Vec<Preset>,
}

impl Default for NodePhysicsApp {
    fn default() -> Self {
        let default_node = demo_node(false, false);
        Self {
            node: default_node,
            samples_per_side: 100,
            presets: preset_nodes(),
        }
    }
}

impl eframe::App for NodePhysicsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("node_controls")
            .min_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Node Profile");
                ui.label("Adjust the profile parameters to update the shell in real time.");
                ui.separator();

                ui.label("Presets");
                ui.horizontal_wrapped(|ui| {
                    for preset in &self.presets {
                        if ui.button(preset.name).clicked() {
                            self.node = preset.node;
                        }
                    }
                });
                if ui.button("Log NodePhysics to terminal").clicked() {
                    println!("{:#?}", self.node);
                }

                let mut controls = self.node.controls();

                ui.separator();
                ui.label("Fringe + material");
                ui.add(
                    egui::Slider::new(&mut controls.fringe.length_m, 0.01..=3.0)
                        .text("fringe.length_m"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.material.wall_thickness_m, 0.0..=0.5)
                        .text("material.wall_thickness_m"),
                );

                ui.separator();
                ui.label("Base");
                ui.add(
                    egui::Slider::new(&mut controls.base.radius_m, 0.0..=1.5).text("base.radius_m"),
                );

                ui.separator();
                ui.label("Shoulder");
                ui.add(
                    egui::Slider::new(&mut controls.shoulder.radius_m, 0.0..=1.5)
                        .text("shoulder.radius_m"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.shoulder.sweep_deg, -180.0..=180.0)
                        .text("shoulder.sweep_deg"),
                );
                ui.checkbox(
                    &mut controls.shoulder.curves_inward,
                    "shoulder.curves_inward",
                );

                ui.separator();
                ui.label("Rim");
                ui.add(
                    egui::Slider::new(&mut controls.rim.radius_m, 0.0..=1.5).text("rim.radius_m"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.rim.sweep_deg, -180.0..=180.0)
                        .text("rim.sweep_deg"),
                );
                ui.add(
                    egui::Slider::new(&mut controls.rim.breadth_m, 0.0..=1.0).text("rim.breadth_m"),
                );
                ui.checkbox(&mut controls.rim.curves_inward, "rim.curves_inward");

                ui.separator();
                ui.label("Bow");
                ui.add(
                    egui::Slider::new(&mut controls.bow.thickness_m, 0.0..=3.0)
                        .text("bow.thickness_m"),
                );
                ui.checkbox(&mut controls.bow.on_inside, "bow.on_inside");

                ui.separator();
                ui.label("Material metadata");
                ui.add(
                    egui::DragValue::new(&mut controls.material.density_kg_per_m3)
                        .speed(1.0)
                        .range(0.0..=100_000.0)
                        .prefix("density "),
                );

                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.samples_per_side, 8..=500).text("samples_per_side"),
                );

                sanitize_controls(&mut controls);
                self.node.apply_controls(controls);

                let computed_samples = self.samples_per_side.max(1);
                let inner_volume_m3 = self.node.inner_volume_m3(computed_samples);
                let material_volume_m3 = self.node.material_volume_m3(computed_samples);
                let mass_kg = self.node.mass_kg();

                let (thickness_start, thickness_mid, thickness_end) = (
                    self.node.thickness_at(0.0),
                    self.node.thickness_at(0.5),
                    self.node.thickness_at(1.0),
                );
                ui.separator();
                ui.label(format!("inner_volume_m3 {:.6}", inner_volume_m3));
                ui.label(format!("material_volume_m3 {:.6}", material_volume_m3));
                ui.label(format!("mass_kg {:.4}", mass_kg));
                ui.label(format!(
                    "thickness: u=0.0 -> {:.4}, u=0.5 -> {:.4}, u=1.0 -> {:.4}",
                    thickness_start, thickness_mid, thickness_end
                ));
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Outline preview");
            ui.label("Outer + inner shell generated by NodePhysics::shell_outline().");
            ui.add_space(8.0);

            let desired = ui.available_size();
            let (response, painter) = ui.allocate_painter(desired, Sense::hover());
            let rect = response.rect;

            painter.rect_filled(rect, 6.0, Color32::from_rgb(22, 24, 30));

            let outline = self.node.shell_outline(self.samples_per_side.max(1));
            if outline.len() < 2 {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No points to render",
                    egui::TextStyle::Body.resolve(ui.style()),
                    Color32::LIGHT_RED,
                );
                return;
            }

            if let Some(to_screen) = make_to_screen(&outline, rect.shrink(24.0)) {
                draw_axes(&painter, &outline, &to_screen);

                let polyline: Vec<Pos2> = outline.iter().map(|p| to_screen(p)).collect();
                painter.add(Shape::line(
                    polyline,
                    Stroke::new(2.0_f32, Color32::from_rgb(110, 228, 154)),
                ));

                let marker_every = (outline.len() / 40).max(1);
                for (idx, p) in outline.iter().enumerate() {
                    if idx % marker_every == 0 {
                        painter.circle_filled(
                            to_screen(p),
                            2.0_f32,
                            Color32::from_rgb(245, 203, 92),
                        );
                    }
                }
            }
        });

        ctx.request_repaint();
    }
}

fn sanitize_controls(controls: &mut NodeControls) {
    controls.material.density_kg_per_m3 = controls.material.density_kg_per_m3.max(0.0);
    controls.material.wall_thickness_m = controls.material.wall_thickness_m.max(0.0);
    controls.base.radius_m = controls.base.radius_m.max(0.0);
    controls.shoulder.radius_m = controls.shoulder.radius_m.max(0.0);
    controls.shoulder.sweep_deg = controls.shoulder.sweep_deg.clamp(-180.0, 180.0);
    controls.fringe.length_m = controls.fringe.length_m.max(0.01);
    controls.rim.radius_m = controls.rim.radius_m.max(0.0);
    controls.rim.sweep_deg = controls.rim.sweep_deg.clamp(-180.0, 180.0);
    controls.rim.breadth_m = controls.rim.breadth_m.max(0.0);
    controls.bow.thickness_m = controls.bow.thickness_m.max(0.0);
}

fn make_to_screen(
    points: &[Point2<f64>],
    rect: Rect,
) -> Option<Box<dyn Fn(&Point2<f64>) -> Pos2 + Send + Sync>> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for p in points {
        if !p.x.is_finite() || !p.y.is_finite() {
            continue;
        }
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }

    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return None;
    }

    let width = (max_x - min_x).max(1e-6);
    let height = (max_y - min_y).max(1e-6);
    let scale_x = rect.width() as f64 / width;
    let scale_y = rect.height() as f64 / height;
    let scale = (scale_x.min(scale_y) * 0.92).max(1e-6);

    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let screen_center = rect.center();

    Some(Box::new(move |p: &Point2<f64>| {
        let x = (p.x - center_x) * scale + screen_center.x as f64;
        let y = (p.y - center_y) * scale + screen_center.y as f64;
        Pos2::new(x as f32, y as f32)
    }))
}

fn draw_axes(
    painter: &egui::Painter,
    points: &[Point2<f64>],
    to_screen: &dyn Fn(&Point2<f64>) -> Pos2,
) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }

    let pad_x = ((max_x - min_x) * 0.1).max(0.05);
    let pad_y = ((max_y - min_y) * 0.1).max(0.05);
    min_x -= pad_x;
    max_x += pad_x;
    min_y -= pad_y;
    max_y += pad_y;

    if min_x <= 0.0 && max_x >= 0.0 {
        let a = to_screen(&Point2 { x: 0.0, y: min_y });
        let b = to_screen(&Point2 { x: 0.0, y: max_y });
        painter.line_segment([a, b], Stroke::new(1.0_f32, Color32::from_gray(95)));
    }

    if min_y <= 0.0 && max_y >= 0.0 {
        let a = to_screen(&Point2 { x: min_x, y: 0.0 });
        let b = to_screen(&Point2 { x: max_x, y: 0.0 });
        painter.line_segment([a, b], Stroke::new(1.0_f32, Color32::from_gray(95)));
    }
}

fn demo_node(shoulder_curves_inward: bool, rim_curves_inward: bool) -> NodePhysics {
    NodePhysics::from_controls(NodeControls {
        base: BaseControls { radius_m: 0.25 },
        shoulder: ShoulderControls {
            radius_m: 0.2,
            sweep_deg: 35.0,
            curves_inward: shoulder_curves_inward,
        },
        fringe: FringeControls { length_m: 1.0 },
        rim: RimControls {
            radius_m: 0.18,
            sweep_deg: 35.0,
            breadth_m: 0.1,
            curves_inward: rim_curves_inward,
        },
        bow: BowControls {
            thickness_m: 0.0,
            on_inside: true,
        },
        material: MaterialControls {
            density_kg_per_m3: 1_000.0,
            wall_thickness_m: 0.05,
        },
    })
}

fn with_controls(base: NodePhysics, f: impl FnOnce(&mut NodeControls)) -> NodePhysics {
    let mut controls = base.controls();
    f(&mut controls);
    NodePhysics::from_controls(controls)
}

fn preset_nodes() -> Vec<Preset> {
    vec![
        Preset {
            name: "bowl_up",
            node: with_controls(demo_node(false, false), |c| {
                c.base.radius_m = 0.22;
                c.shoulder.radius_m = 0.24;
                c.shoulder.sweep_deg = -45.0;
                c.shoulder.curves_inward = true;
                c.rim.radius_m = 0.14;
                c.rim.sweep_deg = -42.0;
                c.rim.breadth_m = 0.10;
                c.rim.curves_inward = false;
                c.fringe.length_m = 2.04;
            }),
        },
        Preset {
            name: "bell_down",
            node: with_controls(demo_node(false, false), |c| {
                c.base.radius_m = 0.24;
                c.shoulder.radius_m = 0.16;
                c.shoulder.sweep_deg = 58.0;
                c.shoulder.curves_inward = false;
                c.rim.radius_m = 0.20;
                c.rim.sweep_deg = 52.0;
                c.rim.breadth_m = 0.10;
                c.rim.curves_inward = true;
                c.fringe.length_m = 1.10;
            }),
        },
        Preset {
            name: "rim_out",
            node: with_controls(demo_node(false, false), |c| {
                c.bow.thickness_m = 0.10;
                c.bow.on_inside = false;
                c.base.radius_m = 0.24;
                c.shoulder.radius_m = 0.16;
                c.shoulder.sweep_deg = 56.0;
                c.shoulder.curves_inward = false;
                c.rim.radius_m = 0.24;
                c.rim.sweep_deg = 82.0;
                c.rim.breadth_m = 0.14;
                c.rim.curves_inward = false;
                c.fringe.length_m = 1.16;
            }),
        },
        Preset {
            name: "rim_in",
            node: with_controls(demo_node(false, false), |c| {
                c.bow.thickness_m = 0.10;
                c.bow.on_inside = true;
                c.base.radius_m = 0.22;
                c.shoulder.radius_m = 0.22;
                c.shoulder.sweep_deg = -44.0;
                c.shoulder.curves_inward = true;
                c.rim.radius_m = 0.20;
                c.rim.sweep_deg = -72.0;
                c.rim.breadth_m = 0.10;
                c.rim.curves_inward = true;
                c.fringe.length_m = 1.06;
            }),
        },
        Preset {
            name: "gong",
            node: with_controls(demo_node(false, false), |c| {
                c.material.wall_thickness_m = 0.04;
                c.base.radius_m = 0.28;
                c.shoulder.radius_m = 0.36;
                c.shoulder.sweep_deg = 12.0;
                c.shoulder.curves_inward = false;
                c.rim.radius_m = 0.34;
                c.rim.sweep_deg = 14.0;
                c.rim.breadth_m = 0.07;
                c.rim.curves_inward = false;
                c.fringe.length_m = 1.14;
            }),
        },
        Preset {
            name: "plate",
            node: with_controls(demo_node(false, false), |c| {
                c.material.wall_thickness_m = 0.025;
                c.base.radius_m = 0.32;
                c.shoulder.radius_m = 0.44;
                c.shoulder.sweep_deg = -6.0;
                c.shoulder.curves_inward = true;
                c.rim.radius_m = 0.42;
                c.rim.sweep_deg = -8.0;
                c.rim.breadth_m = 0.05;
                c.rim.curves_inward = false;
                c.fringe.length_m = 1.20;
            }),
        },
    ]
}
