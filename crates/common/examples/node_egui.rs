// #![allow(clippy::cast_lossless)]
// use common::config::materials::Material;
// use common::config::{Node, Segment};
// use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke};
// use mint::Point2;

// // ─── constants ───────────────────────────────────────────────────────────────

// const SEGMENT_NAMES: [&str; 5] = ["Crown", "Upper body", "Mid-body", "Shoulder", "Rim"];
// const INNER_COLOR: Color32 = Color32::from_rgb(102, 178, 255);
// const OUTER_COLOR: Color32 = Color32::from_rgb(80, 200, 130);
// const BASELINE_COLOR: Color32 = Color32::from_rgb(245, 203, 92);
// const BG_COLOR: Color32 = Color32::from_rgb(18, 20, 26);
// const AXIS_COLOR: Color32 = Color32::from_rgba_premultiplied(100, 100, 120, 60);
// const MIRROR_INNER_COLOR: Color32 = Color32::from_rgba_premultiplied(60, 70, 80, 10);
// const MIRROR_OUTER_COLOR: Color32 = Color32::from_rgba_premultiplied(60, 70, 80, 10);
// const MIRROR_WALL_COLOR: Color32 = Color32::from_rgba_premultiplied(60, 70, 80, 10);

// // ─── segment config widget ────────────────────────────────────────────────────

// #[derive(Clone, Copy, PartialEq, Debug)]
// enum CurveKind {
//     Line,
//     Constant,
//     Parabolic,
//     Hyperbolic,
//     ArcSweep,
// }

// impl CurveKind {
//     fn label(self) -> &'static str {
//         match self {
//             Self::Line => "Line",
//             Self::Constant => "Constant",
//             Self::Parabolic => "Parabolic",
//             Self::Hyperbolic => "Hyperbolic",
//             Self::ArcSweep => "ArcSweep",
//         }
//     }
// }

// #[derive(Clone, Debug)]
// struct SegmentConfig {
//     /// Axial start parameter (fixed from previous segment end, except for first segment).
//     start: f64,
//     /// Axial end parameter (editable).
//     end: f64,
//     density: f64,
//     kind: CurveKind,
//     // Line
//     line_m: f64,
//     line_b: f64,
//     // Constant
//     const_value: f64,
//     // Parabolic
//     para_a: f64,
//     para_b: f64,
//     para_c: f64,
//     // Hyperbolic
//     hyp_a: f64,
//     hyp_b: f64,
//     hyp_c: f64,
//     // ArcSweep
//     arc_center_y: f64,
//     arc_radius: f64,
//     arc_start_angle: f64,
//     arc_sweep_angle: f64,
//     arc_concave_up: bool,
// }

// impl SegmentConfig {
//     fn line_slope_dr_dy(&self) -> f64 {
//         // UI exposes dy/dr for intuitive "outwardness" control.
//         // Internal Segment::Line expects dr/dy.
//         let dy_dr = self.line_m;
//         if dy_dr.abs() < 1e-3 {
//             if dy_dr.is_sign_negative() {
//                 -1_000.0
//             } else {
//                 1_000.0
//             }
//         } else {
//             1.0 / dy_dr
//         }
//     }

//     fn with_start(start: f64, end: f64) -> Self {
//         // Default: a gentle parabolic dome outward (r increases then levels off).
//         let span = (end - start).max(0.01);
//         Self {
//             start,
//             end,
//             density: 12.0,
//             kind: CurveKind::Parabolic,
//             line_m: 0.1,
//             line_b: 0.1,
//             const_value: 0.2,
//             para_a: 0.02 / span,
//             para_b: 0.0,
//             para_c: 0.1,
//             hyp_a: 0.1,
//             hyp_b: start - 0.5,
//             hyp_c: 0.1,
//             arc_center_y: 0.0,
//             arc_radius: span,
//             arc_start_angle: 0.0,
//             arc_sweep_angle: std::f64::consts::FRAC_PI_4,
//             arc_concave_up: false,
//         }
//     }

//     /// Build the first segment using the start constructor family.
//     fn build_start(&self) -> Result<Segment, String> {
//         if self.end <= self.start {
//             return Err(format!(
//                 "end ({:.3}) must be > start ({:.3})",
//                 self.end, self.start
//             ));
//         }
//         match self.kind {
//             CurveKind::Line => {
//                 Segment::start_line(self.end - self.start, self.line_slope_dr_dy(), self.line_b)
//             }
//             CurveKind::Constant => Segment::start_constant(self.end - self.start, self.const_value),
//             CurveKind::Parabolic => Segment::start_parabolic(
//                 self.end - self.start,
//                 self.para_a,
//                 self.para_b,
//                 self.para_c,
//             ),
//             CurveKind::Hyperbolic => {
//                 Segment::start_hyperbolic(self.end - self.start, self.hyp_a, self.hyp_b, self.hyp_c)
//             }
//             CurveKind::ArcSweep => Segment::start_arc_sweep(
//                 self.end - self.start,
//                 self.arc_center_y,
//                 self.arc_radius,
//                 self.arc_start_angle,
//                 self.arc_sweep_angle,
//             ),
//         }
//         .map_err(|e| e.to_string())
//     }

//     /// Build a continuation segment preserving continuity from previous segment.
//     fn build_continuation(&self, prev: &Segment) -> Result<Segment, String> {
//         if self.end <= prev.end {
//             return Err(format!(
//                 "end ({:.3}) must be > previous end ({:.3})",
//                 self.end, prev.end
//             ));
//         }
//         match self.kind {
//             CurveKind::Line => Segment::continue_line(prev, self.end, self.density),
//             CurveKind::Constant => Segment::continue_constant(prev, self.end, self.density),
//             CurveKind::Parabolic => {
//                 Segment::continue_parabolic(prev, self.end, self.const_value, self.density)
//             }
//             CurveKind::Hyperbolic => {
//                 Segment::continue_hyperbolic(prev, self.end, self.hyp_b, self.density)
//             }
//             CurveKind::ArcSweep => Segment::continue_arc_sweep(
//                 prev,
//                 self.end,
//                 self.arc_radius,
//                 self.arc_sweep_angle,
//                 self.arc_concave_up,
//                 self.density,
//             ),
//         }
//         .map_err(|e| e.to_string())
//     }

//     fn show(&mut self, ui: &mut egui::Ui, start_fixed: bool, continuation_mode: bool) {
//         egui::Grid::new(ui.id().with("seg_grid"))
//             .num_columns(2)
//             .show(ui, |ui| {
//                 if start_fixed {
//                     ui.label("start");
//                     ui.monospace(format!("{:.4}", self.start));
//                     ui.end_row();
//                 } else {
//                     ui.label("start");
//                     ui.add(egui::DragValue::new(&mut self.start).speed(0.01));
//                     ui.end_row();
//                 }
//                 ui.label("end");
//                 ui.add(egui::DragValue::new(&mut self.end).speed(0.01));
//                 ui.end_row();
//                 ui.label("density");
//                 ui.add(egui::Slider::new(&mut self.density, 1.0..=64.0));
//                 ui.end_row();
//             });

//         ui.separator();
//         egui::ComboBox::from_id_salt(ui.id().with("kind"))
//             .selected_text(self.kind.label())
//             .show_ui(ui, |ui| {
//                 for k in [
//                     CurveKind::Line,
//                     CurveKind::Constant,
//                     CurveKind::Parabolic,
//                     CurveKind::Hyperbolic,
//                     CurveKind::ArcSweep,
//                 ] {
//                     ui.selectable_value(&mut self.kind, k, k.label());
//                 }
//             });

//         ui.separator();
//         match self.kind {
//             CurveKind::Line => {
//                 if continuation_mode {
//                     ui.small("line continuation keeps C1 continuity from previous segment");
//                 } else {
//                     ui.add(egui::Slider::new(&mut self.line_m, -5.0..=5.0).text("dy/dr (slope)"));
//                     ui.add(egui::Slider::new(&mut self.line_b, -2.0..=2.0).text("r(y=0)"));
//                     ui.small(format!("internal dr/dy = {:.3}", self.line_slope_dr_dy()));
//                 }
//             }
//             CurveKind::Constant => {
//                 if continuation_mode {
//                     ui.small("constant continuation requires near-zero previous slope");
//                 } else {
//                     ui.add(
//                         egui::Slider::new(&mut self.const_value, -2.0..=2.0).text("value (radius)"),
//                     );
//                 }
//             }
//             CurveKind::Parabolic => {
//                 if continuation_mode {
//                     ui.add(
//                         egui::Slider::new(&mut self.const_value, -2.0..=2.0)
//                             .text("to_value (end radius)"),
//                     );
//                 } else {
//                     ui.add(egui::Slider::new(&mut self.para_a, -2.0..=2.0).text("a"));
//                     ui.add(egui::Slider::new(&mut self.para_b, -5.0..=5.0).text("b"));
//                     ui.add(egui::Slider::new(&mut self.para_c, -2.0..=2.0).text("c"));
//                 }
//             }
//             CurveKind::Hyperbolic => {
//                 if continuation_mode {
//                     ui.add(
//                         egui::DragValue::new(&mut self.hyp_b)
//                             .speed(0.05)
//                             .prefix("pole_t "),
//                     );
//                 } else {
//                     ui.add(egui::Slider::new(&mut self.hyp_a, -2.0..=2.0).text("a"));
//                     ui.add(
//                         egui::DragValue::new(&mut self.hyp_b)
//                             .speed(0.05)
//                             .prefix("b (pole) "),
//                     );
//                     ui.add(egui::Slider::new(&mut self.hyp_c, -2.0..=2.0).text("c"));
//                 }
//                 if self.hyp_b > self.start && self.hyp_b < self.end {
//                     ui.colored_label(
//                         Color32::from_rgb(220, 180, 60),
//                         "pole must be outside [start, end]",
//                     );
//                 }
//             }
//             CurveKind::ArcSweep => {
//                 ui.add(egui::Slider::new(&mut self.arc_center_y, -5.0..=5.0).text("center_y"));
//                 ui.add(egui::Slider::new(&mut self.arc_radius, 0.05..=5.0).text("radius"));
//                 ui.add(
//                     egui::Slider::new(
//                         &mut self.arc_sweep_angle,
//                         -std::f64::consts::TAU..=std::f64::consts::TAU,
//                     )
//                     .text("sweep_angle (rad)"),
//                 );
//                 if continuation_mode {
//                     ui.checkbox(&mut self.arc_concave_up, "concave_up");
//                 } else {
//                     ui.add(
//                         egui::Slider::new(
//                             &mut self.arc_start_angle,
//                             -std::f64::consts::PI..=std::f64::consts::PI,
//                         )
//                         .text("start_angle (rad)"),
//                     );
//                 }
//             }
//         }
//     }
// }

// // ─── presets ─────────────────────────────────────────────────────────────────

// struct Preset {
//     #[allow(dead_code)]
//     name: &'static str,
//     segments: [SegmentConfig; 5],
//     wall_thickness_m: [f64; 5],
//     material_density_kg_per_m3: f64,
// }

// fn bell_preset() -> Preset {
//     // A simple bell profile: crown dome, widening body, shoulder flare, rim.
//     let seg = |start: f64, end: f64, a: f64, b: f64, c: f64| -> SegmentConfig {
//         SegmentConfig {
//             start,
//             end,
//             kind: CurveKind::Parabolic,
//             para_a: a,
//             para_b: b,
//             para_c: c,
//             ..SegmentConfig::with_start(start, end)
//         }
//     };
//     Preset {
//         name: "bell",
//         material_density_kg_per_m3: 8_500.0, // bronze
//         wall_thickness_m: [0.006, 0.007, 0.008, 0.010, 0.015],
//         segments: [
//             seg(0.0, 0.10, 0.0, 0.5, 0.05),   // crown: spreads outward
//             seg(0.10, 0.35, 0.5, 0.0, 0.05),  // upper body: widens
//             seg(0.35, 0.65, 0.0, 0.45, 0.0),  // mid-body: broad
//             seg(0.65, 0.85, 0.8, -0.8, 0.35), // shoulder: flares out
//             seg(0.85, 1.00, 0.0, 0.1, 0.45),  // rim: vertical edge
//         ],
//     }
// }

// fn gong_preset() -> Preset {
//     let seg = |start: f64, end: f64, a: f64, b: f64, c: f64| -> SegmentConfig {
//         SegmentConfig {
//             start,
//             end,
//             kind: CurveKind::Parabolic,
//             para_a: a,
//             para_b: b,
//             para_c: c,
//             ..SegmentConfig::with_start(start, end)
//         }
//     };
//     Preset {
//         name: "gong",
//         material_density_kg_per_m3: 8_000.0,
//         wall_thickness_m: [0.003, 0.003, 0.004, 0.005, 0.012],
//         segments: [
//             seg(0.0, 0.05, 0.0, 0.4, 0.02),  // crown: spreads outward gently
//             seg(0.05, 0.20, 0.2, 0.0, 0.02), // gentle rise
//             seg(0.20, 0.50, 0.0, 0.4, 0.0),  // broad flat middle
//             seg(0.50, 0.80, 0.5, -0.5, 0.3), // shoulder
//             seg(0.80, 1.00, 0.0, 0.05, 0.4), // rim
//         ],
//     }
// }

// // ─── app state ────────────────────────────────────────────────────────────────

// struct NodeApp {
//     segments: [SegmentConfig; 5],
//     wall_thickness_m: [f64; 5],
//     material_density_kg_per_m3: f64,
//     poisson_ratio: f64,
//     youngs_modulus_pa: f64,
//     inner_soundbow_enabled: bool,
//     outer_soundbow_enabled: bool,
//     inner_soundbow: SegmentConfig,
//     outer_soundbow: SegmentConfig,
//     samples: usize,
//     expanded_section: Option<usize>,
// }

// impl Default for NodeApp {
//     fn default() -> Self {
//         let preset = bell_preset();
//         Self {
//             wall_thickness_m: preset.wall_thickness_m,
//             material_density_kg_per_m3: preset.material_density_kg_per_m3,
//             poisson_ratio: 0.34,
//             youngs_modulus_pa: 110e9,
//             inner_soundbow_enabled: false,
//             outer_soundbow_enabled: false,
//             inner_soundbow: SegmentConfig::with_start(0.80, 1.00),
//             outer_soundbow: SegmentConfig::with_start(0.80, 1.00),
//             samples: 80,
//             expanded_section: None,
//             segments: preset.segments,
//         }
//     }
// }

// impl NodeApp {
//     fn apply_preset(&mut self, preset: Preset) {
//         self.segments = preset.segments;
//         self.wall_thickness_m = preset.wall_thickness_m;
//         self.material_density_kg_per_m3 = preset.material_density_kg_per_m3;
//     }

//     fn chain_starts(&mut self) {
//         self.segments[0].start = 0.0;
//         for i in 1..5 {
//             let prev_end = self.segments[i - 1].end;
//             self.segments[i].start = prev_end;
//         }
//     }

//     fn build_node(&mut self) -> Result<Node, String> {
//         self.chain_starts();
//         let mut segs_built = [None; 5];
//         segs_built[0] = Some(
//             self.segments[0]
//                 .build_start()
//                 .map_err(|e| format!("{}: {e}", SEGMENT_NAMES[0]))?,
//         );

//         for i in 1..5 {
//             let prev = segs_built[i - 1].unwrap();
//             segs_built[i] = Some(
//                 self.segments[i]
//                     .build_continuation(&prev)
//                     .map_err(|e| format!("{}: {e}", SEGMENT_NAMES[i]))?,
//             );
//             self.segments[i].start = prev.end;
//             self.segments[i].end = segs_built[i].unwrap().end;
//         }
//         let profile_segments = [
//             segs_built[0].unwrap(),
//             segs_built[1].unwrap(),
//             segs_built[2].unwrap(),
//             segs_built[3].unwrap(),
//             segs_built[4].unwrap(),
//         ];
//         let wall_thickness_m = self.wall_thickness_m;

//         let inner_bow = if self.inner_soundbow_enabled {
//             Some(
//                 self.inner_soundbow
//                     .build_start()
//                     .map_err(|e| format!("inner soundbow: {e}"))?,
//             )
//         } else {
//             None
//         };
//         let outer_bow = if self.outer_soundbow_enabled {
//             Some(
//                 self.outer_soundbow
//                     .build_start()
//                     .map_err(|e| format!("outer soundbow: {e}"))?,
//             )
//         } else {
//             None
//         };

//         Ok(Node {
//             material: Material {
//                 material_density_kg_per_m3: self.material_density_kg_per_m3,
//                 poisson_ratio: self.poisson_ratio,
//                 youngs_modulus_pa: self.youngs_modulus_pa,
//             },
//             profile_segments,
//             wall_thickness_m,
//             soundbow_segments: [inner_bow, outer_bow],
//         })
//     }
// }

// // ─── side panel ──────────────────────────────────────────────────────────────

// fn show_controls(app: &mut NodeApp, ui: &mut egui::Ui, node_result: &Result<Node, String>) {
//     // Presets
//     ui.heading("Presets");
//     ui.horizontal_wrapped(|ui| {
//         if ui.button("bell").clicked() {
//             app.apply_preset(bell_preset());
//         }
//         if ui.button("gong").clicked() {
//             app.apply_preset(gong_preset());
//         }
//     });
//     ui.separator();

//     // Material
//     ui.heading("Material");
//     egui::Grid::new("material_grid")
//         .num_columns(2)
//         .striped(true)
//         .show(ui, |ui| {
//             ui.label("density (kg/m³)");
//             ui.add(
//                 egui::DragValue::new(&mut app.material_density_kg_per_m3)
//                     .speed(10.0)
//                     .range(100.0..=20_000.0),
//             );
//             ui.end_row();
//             ui.label("Poisson ratio");
//             ui.add(egui::Slider::new(&mut app.poisson_ratio, 0.0..=0.5));
//             ui.end_row();
//             ui.label("Young's modulus (Pa)");
//             ui.add(
//                 egui::DragValue::new(&mut app.youngs_modulus_pa)
//                     .speed(1e9)
//                     .range(1e9..=500e9),
//             );
//             ui.end_row();
//         });
//     ui.separator();

//     // Profile segments
//     ui.heading("Profile segments");
//     ui.small("Segment 1 uses start_*; segments 2-5 use continue_* from the previous segment.");
//     for i in 0..5 {
//         let is_open = app.expanded_section == Some(i);
//         let header = format!(
//             "{}  [t {:.3}–{:.3}]  wall {:.4} m",
//             SEGMENT_NAMES[i], app.segments[i].start, app.segments[i].end, app.wall_thickness_m[i]
//         );
//         let resp = egui::CollapsingHeader::new(header)
//             .id_salt(format!("seg_{i}"))
//             .open(if is_open { Some(true) } else { None })
//             .show(ui, |ui| {
//                 app.segments[i].show(ui, true, i != 0);
//                 ui.separator();
//                 ui.add(
//                     egui::Slider::new(&mut app.wall_thickness_m[i], 0.001..=0.1)
//                         .text("wall thickness (m)"),
//                 );
//             });
//         if resp.header_response.clicked() {
//             app.expanded_section = if is_open { None } else { Some(i) };
//         }
//     }
//     ui.separator();

//     // Soundbow
//     ui.heading("Soundbow");
//     ui.checkbox(&mut app.inner_soundbow_enabled, "Inner soundbow (concave)");
//     if app.inner_soundbow_enabled {
//         egui::CollapsingHeader::new("Inner soundbow segment")
//             .id_salt("inner_bow")
//             .show(ui, |ui| {
//                 app.inner_soundbow.show(ui, false, false);
//             });
//     }
//     ui.checkbox(&mut app.outer_soundbow_enabled, "Outer soundbow (convex)");
//     if app.outer_soundbow_enabled {
//         egui::CollapsingHeader::new("Outer soundbow segment")
//             .id_salt("outer_bow")
//             .show(ui, |ui| {
//                 app.outer_soundbow.show(ui, false, false);
//             });
//     }
//     ui.separator();

//     // Samples
//     ui.add(egui::Slider::new(&mut app.samples, 16..=400).text("sampling budget"));
//     ui.separator();

//     // Computed stats
//     ui.heading("Computed");
//     match node_result {
//         Err(e) => {
//             ui.colored_label(Color32::LIGHT_RED, format!("Error: {e}"));
//         }
//         Ok(node) => {
//             let n = app.samples.max(2);
//             let sz = node.size(n);
//             let mass = node.mass_kg(n);
//             let mat_vol = node.material_volume_m3(n);
//             let inn_vol = node.inner_volume_m3(n);
//             let t0 = node.thickness_at(0.0);
//             let t50 = node.thickness_at(0.5);
//             let t100 = node.thickness_at(1.0);
//             egui::Grid::new("stats_grid")
//                 .num_columns(2)
//                 .striped(true)
//                 .show(ui, |ui| {
//                     ui.label("size (m)");
//                     ui.monospace(format!("{:.4} × {:.4} × {:.4}", sz.x, sz.y, sz.z));
//                     ui.end_row();
//                     ui.label("mass (kg)");
//                     ui.monospace(format!("{:.4}", mass));
//                     ui.end_row();
//                     ui.label("material vol (m³)");
//                     ui.monospace(format!("{:.6}", mat_vol));
//                     ui.end_row();
//                     ui.label("inner vol (m³)");
//                     ui.monospace(format!("{:.6}", inn_vol));
//                     ui.end_row();
//                     ui.label("thickness u=0");
//                     ui.monospace(format!("{:.5} m", t0));
//                     ui.end_row();
//                     ui.label("thickness u=0.5");
//                     ui.monospace(format!("{:.5} m", t50));
//                     ui.end_row();
//                     ui.label("thickness u=1");
//                     ui.monospace(format!("{:.5} m", t100));
//                     ui.end_row();
//                 });
//         }
//     }
// }

// // ─── central panel ───────────────────────────────────────────────────────────

// fn draw_node(ui: &mut egui::Ui, node_result: &Result<Node, String>, samples: usize) {
//     let desired = ui.available_size();
//     let (response, painter) = ui.allocate_painter(desired, Sense::hover());
//     let rect = response.rect;

//     painter.rect_filled(rect, 4.0, BG_COLOR);

//     let node = match node_result {
//         Err(e) => {
//             painter.text(
//                 rect.center(),
//                 egui::Align2::CENTER_CENTER,
//                 format!("Error: {e}"),
//                 egui::TextStyle::Body.resolve(ui.style()),
//                 Color32::LIGHT_RED,
//             );
//             return;
//         }
//         Ok(n) => n,
//     };

//     let n_profile = (samples * 2).max(40);

//     // Collect contour points with explicit ordering:
//     // inner (l:-1..0) and outer (l:0..1), then mirror both.
//     let mut inner_profile: Vec<Point2<f64>> = Vec::with_capacity(n_profile / 2 + 2);
//     let mut outer_profile: Vec<Point2<f64>> = Vec::with_capacity(n_profile / 2 + 2);
//     let half = n_profile / 2;
//     for i in 0..=half {
//         let t = i as f64 / half as f64;
//         // Inner contour runs crown(-1) -> rim(0); bias samples toward crown.
//         let l = -1.0 + t.powi(3);
//         inner_profile.push(node.profile_at(l));
//     }
//     for i in 0..=half {
//         let t = i as f64 / half as f64;
//         // Outer contour runs rim(0) -> crown(1); bias samples toward crown.
//         let l = 1.0 - (1.0 - t).powi(3);
//         outer_profile.push(node.profile_at(l));
//     }

//     // Collect sample_at_u points for thickness markers.
//     let n_samp = samples.max(10);

//     // Compute bounding box over all profile points (x = radius, y = axial depth).
//     // For display: x-axis is horizontal (mirrored: -r .. 0 .. +r), y-axis is vertical (0 at top).
//     let mut max_r = 0.0_f64;
//     let mut max_y = 0.0_f64;
//     for p in inner_profile.iter().chain(outer_profile.iter()) {
//         max_r = max_r.max(p.x.abs());
//         max_y = max_y.max(p.y.abs());
//     }
//     max_r = max_r.max(1e-6);
//     max_y = max_y.max(1e-6);

//     let draw_rect = rect.shrink(24.0);
//     let scale_r = draw_rect.width() as f64 / (2.0 * max_r * 1.1);
//     let scale_y = draw_rect.height() as f64 / (max_y * 1.1);
//     let scale = scale_r.min(scale_y);

//     let cx = draw_rect.center().x as f64;
//     let top = draw_rect.top() as f64 + (draw_rect.height() as f64 - max_y * 1.05 * scale) / 2.0;

//     let map_pt =
//         |r: f64, y: f64| -> Pos2 { Pos2::new((cx + r * scale) as f32, (top + y * scale) as f32) };

//     // Symmetry axis.
//     painter.line_segment(
//         [map_pt(0.0, -max_y * 0.05), map_pt(0.0, max_y * 1.1)],
//         Stroke::new(1.0_f32, AXIS_COLOR),
//     );

//     // Build explicit contour loops to avoid accidental long chords.
//     let inner_right: Vec<Pos2> = inner_profile.iter().map(|p| map_pt(p.x, p.y)).collect();
//     let outer_right: Vec<Pos2> = outer_profile.iter().map(|p| map_pt(p.x, p.y)).collect();
//     let inner_left: Vec<Pos2> = inner_profile.iter().map(|p| map_pt(-p.x, p.y)).collect();
//     let outer_left: Vec<Pos2> = outer_profile.iter().map(|p| map_pt(-p.x, p.y)).collect();

//     if inner_right.len() >= 2 {
//         painter.add(Shape::line(inner_right, Stroke::new(2.0_f32, INNER_COLOR)));
//     }
//     if outer_right.len() >= 2 {
//         painter.add(Shape::line(outer_right, Stroke::new(2.0_f32, OUTER_COLOR)));
//     }
//     if inner_left.len() >= 2 {
//         painter.add(Shape::line(
//             inner_left,
//             Stroke::new(1.0_f32, MIRROR_INNER_COLOR),
//         ));
//     }
//     if outer_left.len() >= 2 {
//         painter.add(Shape::line(
//             outer_left,
//             Stroke::new(1.0_f32, MIRROR_OUTER_COLOR),
//         ));
//     }

//     // sample_at_u markers: outer (green), inner (blue), connected by thin lines.
//     let marker_every = (n_samp / 20).max(1);
//     for i in 0..=n_samp {
//         let u = i as f64 / n_samp as f64;
//         let (outer, inner) = node.sample_at_u(u);
//         if i % marker_every == 0 {
//             let op_r = map_pt(outer.x, outer.y);
//             let op_l = map_pt(-outer.x, outer.y);
//             let ip_r = map_pt(inner.x, inner.y);
//             let ip_l = map_pt(-inner.x, inner.y);
//             // Wall thickness lines.
//             painter.line_segment(
//                 [op_r, ip_r],
//                 Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(200, 200, 200, 40)),
//             );
//             painter.line_segment([op_l, ip_l], Stroke::new(1.0_f32, MIRROR_WALL_COLOR));
//             painter.circle_filled(op_r, 2.5_f32, OUTER_COLOR);
//             painter.circle_filled(op_l, 2.0_f32, MIRROR_OUTER_COLOR);
//             painter.circle_filled(ip_r, 2.5_f32, INNER_COLOR);
//             painter.circle_filled(ip_l, 2.0_f32, MIRROR_INNER_COLOR);
//         }
//     }

//     // Rim tip marker.
//     let rim = node.profile_at(0.0);
//     let rim_r = map_pt(rim.x, rim.y);
//     let rim_l = map_pt(-rim.x, rim.y);
//     painter.circle_filled(rim_r, 4.0_f32, BASELINE_COLOR);
//     painter.circle_filled(
//         rim_l,
//         3.0_f32,
//         Color32::from_rgba_premultiplied(245, 203, 92, 90),
//     );

//     // Crown origin marker.
//     let crown = map_pt(0.0, 0.0);
//     painter.circle_filled(crown, 4.0_f32, Color32::WHITE);

//     // Legend.
//     let font = egui::TextStyle::Small.resolve(ui.style());
//     painter.text(
//         Pos2::new(draw_rect.left() + 4.0, draw_rect.top() + 4.0),
//         egui::Align2::LEFT_TOP,
//         "● inner   ● outer   ● rim   ○ crown",
//         font.clone(),
//         Color32::from_gray(160),
//     );
// }

// // ─── eframe App ──────────────────────────────────────────────────────────────

// impl eframe::App for NodeApp {
//     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
//         let node_result = self.build_node();
//         let samples = self.samples;

//         egui::SidePanel::left("node_controls")
//             .min_width(300.0)
//             .max_width(420.0)
//             .show(ctx, |ui| {
//                 egui::ScrollArea::vertical().show(ui, |ui| {
//                     show_controls(self, ui, &node_result);
//                 });
//             });

//         egui::CentralPanel::default().show(ctx, |ui| {
//             draw_node(ui, &node_result, samples);
//         });

//         ctx.request_repaint();
//     }
// }

// // ─── main ─────────────────────────────────────────────────────────────────────
fn main() {

    //     let options = eframe::NativeOptions {
    //         viewport: egui::ViewportBuilder::default()
    //             .with_title("Node preview")
    //             .with_inner_size([1200.0, 800.0]),
    //         ..Default::default()
    //     };
    //     eframe::run_native(
    //         "Node preview",
    //         options,
    //         Box::new(|_cc| Ok(Box::new(NodeApp::default()))),
    //     )
}
