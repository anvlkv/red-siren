// use common::config::NodePhysics;
// use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke};
// use mint::Point2;

fn main() /*-> eframe::Result<()> */
{
    //     let options = eframe::NativeOptions {
    //         viewport: egui::ViewportBuilder::default()
    //             .with_title("NodePhysics egui preview")
    //             .with_inner_size([1200.0, 760.0]),
    //         ..Default::default()
    //     };

    //     eframe::run_native(
    //         "NodePhysics egui preview",
    //         options,
    //         Box::new(|_cc| Ok(Box::new(NodePhysicsApp::default()))),
    //     )
}

// #[derive(Clone, Copy)]
// struct Preset {
//     name: &'static str,
//     node: NodePhysics,
// }

// struct NodePhysicsApp {
//     node: NodePhysics,
//     samples_per_side: usize,
//     presets: Vec<Preset>,
// }

// impl Default for NodePhysicsApp {
//     fn default() -> Self {
//         let default_node = demo_node(false, false);
//         Self {
//             node: default_node,
//             samples_per_side: 100,
//             presets: preset_nodes(),
//         }
//     }
// }

// impl eframe::App for NodePhysicsApp {
//     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
//         egui::SidePanel::left("node_controls")
//             .min_width(320.0)
//             .show(ctx, |ui| {
//                 ui.heading("Node Profile");
//                 ui.label("Adjust the profile parameters to update the shell in real time.");
//                 ui.separator();

//                 ui.label("Presets");
//                 ui.horizontal_wrapped(|ui| {
//                     for preset in &self.presets {
//                         if ui.button(preset.name).clicked() {
//                             self.node = preset.node;
//                         }
//                     }
//                 });
//                 if ui.button("Log NodePhysics to terminal").clicked() {
//                     println!("{:#?}", self.node);
//                 }

//                 ui.separator();
//                 ui.label("Profile geometry");
//                 ui.add(
//                     egui::Slider::new(&mut self.node.base_length_m, 0.01..=3.0)
//                         .text("base_length_m"),
//                 );
//                 ui.add(
//                     egui::Slider::new(&mut self.node.wall_thickness_m, 0.0..=0.5)
//                         .text("wall_thickness_m"),
//                 );
//                 ui.add(
//                     egui::Slider::new(&mut self.node.rim_breadth, 0.0..=1.0).text("rim_breadth"),
//                 );

//                 ui.separator();
//                 ui.label("Base + shoulder");
//                 ui.add(
//                     egui::Slider::new(&mut self.node.base_radius_m, 0.0..=1.5)
//                         .text("base_radius_m"),
//                 );
//                 ui.add(
//                     egui::Slider::new(&mut self.node.shoulder_radius_m, 0.0..=1.5)
//                         .text("shoulder_radius_m"),
//                 );
//                 ui.add(
//                     egui::Slider::new(&mut self.node.shoulder_sweep_deg, -180.0..=180.0)
//                         .text("shoulder_sweep_deg"),
//                 );
//                 ui.checkbox(
//                     &mut self.node.shoulder_curves_inward,
//                     "shoulder_curves_inward",
//                 );

//                 ui.separator();
//                 ui.label("Rim");
//                 ui.add(
//                     egui::Slider::new(&mut self.node.rim_radius_m, 0.0..=1.5).text("rim_radius_m"),
//                 );
//                 ui.add(
//                     egui::Slider::new(&mut self.node.rim_sweep_deg, -180.0..=180.0)
//                         .text("rim_sweep_deg"),
//                 );
//                 ui.checkbox(&mut self.node.rim_curves_inward, "rim_curves_inward");

//                 ui.separator();
//                 ui.label("Material metadata");
//                 ui.add(
//                     egui::DragValue::new(&mut self.node.material_density_kg_per_m3)
//                         .speed(1.0)
//                         .range(0.0..=100_000.0)
//                         .prefix("density "),
//                 );

//                 ui.separator();
//                 ui.add(
//                     egui::Slider::new(&mut self.samples_per_side, 8..=500).text("samples_per_side"),
//                 );

//                 sanitize_node(&mut self.node);
//                 let computed_samples = self.samples_per_side.max(1);
//                 let inner_volume_m3 = self.node.inner_volume_m3(computed_samples);
//                 let material_volume_m3 = self.node.material_volume_m3(computed_samples);
//                 let mass_kg = self.node.mass_kg();

//                 let (thickness_start, thickness_mid, thickness_end) = (
//                     self.node.thickness_at(0.0),
//                     self.node.thickness_at(0.5),
//                     self.node.thickness_at(1.0),
//                 );
//                 ui.separator();
//                 ui.label(format!("inner_volume_m3 {:.6}", inner_volume_m3));
//                 ui.label(format!("material_volume_m3 {:.6}", material_volume_m3));
//                 ui.label(format!("mass_kg {:.4}", mass_kg));
//                 ui.label(format!(
//                     "thickness: u=0.0 -> {:.4}, u=0.5 -> {:.4}, u=1.0 -> {:.4}",
//                     thickness_start, thickness_mid, thickness_end
//                 ));
//             });

//         egui::CentralPanel::default().show(ctx, |ui| {
//             ui.heading("Outline preview");
//             ui.label("Outer + inner shell generated by NodePhysics::shell_outline().");
//             ui.add_space(8.0);

//             let desired = ui.available_size();
//             let (response, painter) = ui.allocate_painter(desired, Sense::hover());
//             let rect = response.rect;

//             painter.rect_filled(rect, 6.0, Color32::from_rgb(22, 24, 30));

//             let outline = self.node.shell_outline(self.samples_per_side.max(1));
//             if outline.len() < 2 {
//                 painter.text(
//                     rect.center(),
//                     egui::Align2::CENTER_CENTER,
//                     "No points to render",
//                     egui::TextStyle::Body.resolve(ui.style()),
//                     Color32::LIGHT_RED,
//                 );
//                 return;
//             }

//             if let Some(to_screen) = make_to_screen(&outline, rect.shrink(24.0)) {
//                 draw_axes(&painter, &outline, &to_screen);

//                 let polyline: Vec<Pos2> = outline.iter().map(|p| to_screen(p)).collect();
//                 painter.add(Shape::line(
//                     polyline,
//                     Stroke::new(2.0_f32, Color32::from_rgb(110, 228, 154)),
//                 ));

//                 let marker_every = (outline.len() / 40).max(1);
//                 for (idx, p) in outline.iter().enumerate() {
//                     if idx % marker_every == 0 {
//                         painter.circle_filled(
//                             to_screen(p),
//                             2.0_f32,
//                             Color32::from_rgb(245, 203, 92),
//                         );
//                     }
//                 }
//             }
//         });

//         ctx.request_repaint();
//     }
// }

// fn sanitize_node(node: &mut NodePhysics) {
//     node.material_density_kg_per_m3 = node.material_density_kg_per_m3.max(0.0);
//     node.wall_thickness_m = node.wall_thickness_m.max(0.0);
//     node.rim_breadth = node.rim_breadth.max(0.0);
//     node.base_radius_m = node.base_radius_m.max(0.0);
//     node.shoulder_radius_m = node.shoulder_radius_m.max(0.0);
//     node.rim_radius_m = node.rim_radius_m.max(0.0);
//     node.base_length_m = node.base_length_m.max(0.01);
//     node.shoulder_sweep_deg = node.shoulder_sweep_deg.clamp(-180.0, 180.0);
//     node.rim_sweep_deg = node.rim_sweep_deg.clamp(-180.0, 180.0);
// }

// fn make_to_screen(
//     points: &[Point2<f64>],
//     rect: Rect,
// ) -> Option<Box<dyn Fn(&Point2<f64>) -> Pos2 + Send + Sync>> {
//     let mut min_x = f64::INFINITY;
//     let mut min_y = f64::INFINITY;
//     let mut max_x = f64::NEG_INFINITY;
//     let mut max_y = f64::NEG_INFINITY;

//     for p in points {
//         if !p.x.is_finite() || !p.y.is_finite() {
//             continue;
//         }
//         min_x = min_x.min(p.x);
//         min_y = min_y.min(p.y);
//         max_x = max_x.max(p.x);
//         max_y = max_y.max(p.y);
//     }

//     if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
//         return None;
//     }

//     let width = (max_x - min_x).max(1e-6);
//     let height = (max_y - min_y).max(1e-6);
//     let scale_x = rect.width() as f64 / width;
//     let scale_y = rect.height() as f64 / height;
//     let scale = (scale_x.min(scale_y) * 0.92).max(1e-6);

//     let center_x = (min_x + max_x) * 0.5;
//     let center_y = (min_y + max_y) * 0.5;
//     let screen_center = rect.center();

//     Some(Box::new(move |p: &Point2<f64>| {
//         let x = (p.x - center_x) * scale + screen_center.x as f64;
//         let y = (p.y - center_y) * scale + screen_center.y as f64;
//         Pos2::new(x as f32, y as f32)
//     }))
// }

// fn draw_axes(
//     painter: &egui::Painter,
//     points: &[Point2<f64>],
//     to_screen: &dyn Fn(&Point2<f64>) -> Pos2,
// ) {
//     let mut min_x = f64::INFINITY;
//     let mut min_y = f64::INFINITY;
//     let mut max_x = f64::NEG_INFINITY;
//     let mut max_y = f64::NEG_INFINITY;

//     for p in points {
//         min_x = min_x.min(p.x);
//         min_y = min_y.min(p.y);
//         max_x = max_x.max(p.x);
//         max_y = max_y.max(p.y);
//     }

//     let pad_x = ((max_x - min_x) * 0.1).max(0.05);
//     let pad_y = ((max_y - min_y) * 0.1).max(0.05);
//     min_x -= pad_x;
//     max_x += pad_x;
//     min_y -= pad_y;
//     max_y += pad_y;

//     if min_x <= 0.0 && max_x >= 0.0 {
//         let a = to_screen(&Point2 { x: 0.0, y: min_y });
//         let b = to_screen(&Point2 { x: 0.0, y: max_y });
//         painter.line_segment([a, b], Stroke::new(1.0_f32, Color32::from_gray(95)));
//     }

//     if min_y <= 0.0 && max_y >= 0.0 {
//         let a = to_screen(&Point2 { x: min_x, y: 0.0 });
//         let b = to_screen(&Point2 { x: max_x, y: 0.0 });
//         painter.line_segment([a, b], Stroke::new(1.0_f32, Color32::from_gray(95)));
//     }
// }

// fn demo_node(shoulder_curves_inward: bool, rim_curves_inward: bool) -> NodePhysics {
//     NodePhysics {
//         material_density_kg_per_m3: 1_000.0,
//         wall_thickness_m: 0.05,
//         rim_breadth: 0.1,
//         base_radius_m: 0.25,
//         shoulder_radius_m: 0.2,
//         shoulder_sweep_deg: 35.0,
//         rim_radius_m: 0.18,
//         rim_sweep_deg: 35.0,
//         shoulder_curves_inward,
//         rim_curves_inward,
//         base_length_m: 1.0,
//     }
// }

// fn preset_nodes() -> Vec<Preset> {
//     vec![
//         Preset {
//             name: "bowl_up",
//             node: NodePhysics {
//                 base_radius_m: 0.22,
//                 shoulder_radius_m: 0.24,
//                 rim_radius_m: 0.14,
//                 shoulder_sweep_deg: -45.0,
//                 rim_sweep_deg: -42.0,
//                 rim_breadth: 0.10,
//                 shoulder_curves_inward: true,
//                 rim_curves_inward: false,
//                 base_length_m: 2.04,
//                 ..demo_node(false, false)
//             },
//         },
//         Preset {
//             name: "bell_down",
//             node: NodePhysics {
//                 base_radius_m: 0.24,
//                 shoulder_radius_m: 0.16,
//                 rim_radius_m: 0.20,
//                 shoulder_sweep_deg: 58.0,
//                 rim_sweep_deg: 52.0,
//                 rim_breadth: 0.10,
//                 shoulder_curves_inward: false,
//                 rim_curves_inward: true,
//                 base_length_m: 1.10,
//                 ..demo_node(false, false)
//             },
//         },
//         Preset {
//             name: "rim_out",
//             node: NodePhysics {
//                 base_radius_m: 0.24,
//                 shoulder_radius_m: 0.16,
//                 rim_radius_m: 0.24,
//                 shoulder_sweep_deg: 56.0,
//                 rim_sweep_deg: 82.0,
//                 rim_breadth: 0.14,
//                 shoulder_curves_inward: false,
//                 rim_curves_inward: false,
//                 base_length_m: 1.16,
//                 ..demo_node(false, false)
//             },
//         },
//         Preset {
//             name: "rim_in",
//             node: NodePhysics {
//                 base_radius_m: 0.22,
//                 shoulder_radius_m: 0.22,
//                 rim_radius_m: 0.20,
//                 shoulder_sweep_deg: -44.0,
//                 rim_sweep_deg: -72.0,
//                 rim_breadth: 0.10,
//                 shoulder_curves_inward: true,
//                 rim_curves_inward: true,
//                 base_length_m: 1.06,
//                 ..demo_node(false, false)
//             },
//         },
//         Preset {
//             name: "gong",
//             node: NodePhysics {
//                 wall_thickness_m: 0.04,
//                 rim_breadth: 0.07,
//                 base_radius_m: 0.28,
//                 shoulder_radius_m: 0.36,
//                 rim_radius_m: 0.34,
//                 shoulder_sweep_deg: 12.0,
//                 rim_sweep_deg: 14.0,
//                 shoulder_curves_inward: false,
//                 rim_curves_inward: false,
//                 base_length_m: 1.14,
//                 ..demo_node(false, false)
//             },
//         },
//         Preset {
//             name: "plate",
//             node: NodePhysics {
//                 wall_thickness_m: 0.025,
//                 rim_breadth: 0.05,
//                 base_radius_m: 0.32,
//                 shoulder_radius_m: 0.44,
//                 rim_radius_m: 0.42,
//                 shoulder_sweep_deg: -6.0,
//                 rim_sweep_deg: -8.0,
//                 shoulder_curves_inward: true,
//                 rim_curves_inward: false,
//                 base_length_m: 1.20,
//                 ..demo_node(false, false)
//             },
//         },
//     ]
// }
