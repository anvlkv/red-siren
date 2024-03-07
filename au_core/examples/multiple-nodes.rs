use std::collections::HashSet;

use au_core::{
    fft_cons, snoops_cons, FFTCons, FFTData, Node, SnoopsCons, SnoopsData, Unit, UnitEV,
    FFT_BUF_SIZE, MAX_F, MIN_F, SNOOPS_BUF_SIZE,
};
use eframe::egui::{self, *};
use fundsp::hacker32::*;
use futures::channel::mpsc::unbounded;
use hecs::{Bundle, Entity, World};
use logging_timer::timer;

struct State {
    unit: Unit,
    input: bool,
    f0: f32,
    nodes: Vec<(Entity, Entity)>,
    pressed: HashSet<Entity>,
    world: World,
    last_fft: FFTData,
    last_snoops: SnoopsData,
    fft_cons: &'static mut FFTCons,
    snoops_cons: &'static mut SnoopsCons,
}

#[derive(Bundle, Clone)]
struct Control {
    f: usize,
}

const KEYS: [Key; 10] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
    Key::Num0,
];

fn main() {
    simple_logger::init_with_level(log::Level::Error).expect("couldn't initialize logging");
    let (resolve_sender, resolve_receiver) = unbounded();
    std::mem::forget(resolve_receiver);

    let unit = Unit::new(resolve_sender);
    run(unit).unwrap();
}

fn make_nodes(world: &mut World, count: usize, f0: f32) -> (Vec<(Entity, Entity)>, Vec<Node>) {
    let (nodes, config) = (1..=count).fold((vec![], vec![]), |mut acc, i: usize| {
        let b = world.spawn((Control { f: i },));
        let node = Node::new(b);
        node.f_emit.0.set_value(i as f32 * f0 * 2.0);
        node.f_emit.1.set_value(i as f32 * f0 * 2.75);
        node.f_sense.0 .0.set_value(i as f32 * f0 * 1.5 * 2.0);
        node.f_sense
            .0
             .1
            .set_value((i + 1) as f32 * f0 * 1.5 * 2.75);
        node.f_sense.1 .0.set_value(0.1 * (i + 2) as f32);
        node.f_sense.1 .1.set_value(0.95 - 0.01 * (i + 2) as f32);
        node.pan.set_value(if i % 2 == 0 { -0.75 } else { 0.75 });
        let n = world.spawn((node.clone(),));
        acc.0.push((n, b));
        acc.1.push(node);
        acc
    });

    (nodes, config)
}

fn run(mut unit: Unit) -> Result<(), anyhow::Error> {
    let mut world = World::new();
    let f0 = 85.0;
    let (nodes, config) = make_nodes(&mut world, 4, f0);

    unit.run()?;

    unit.update(UnitEV::Configure(config));

    let state = State {
        unit,
        input: true,
        world,
        nodes,
        fft_cons: fft_cons(),
        snoops_cons: snoops_cons(),
        f0,
        last_fft: vec![],
        last_snoops: vec![],
        pressed: HashSet::new(),
    };
    let viewport = ViewportBuilder::default().with_min_inner_size(vec2(880.0, 600.0));

    let options = eframe::NativeOptions {
        viewport,
        ..eframe::NativeOptions::default()
    };

    eframe::run_native(
        "Multiple nodes example",
        options,
        Box::new(|_cc| Box::new(state)),
    )
    .unwrap();

    Ok(())
}

impl eframe::App for State {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let _tmr = timer!("render tick");
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Multiple nodes example");
            ui.separator();
            ui.end_row();
            egui::ScrollArea::vertical()
                .max_height(600.0 * 0.75)
                .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.label("Input");
                    ui.horizontal(|ui| {
                        let input2 = ui.selectable_value(&mut self.input, true, "Use mic");
                        let input1 = ui.selectable_value(&mut self.input, false, "Use keyboard");
                        if input1.changed() || input2.changed() {
                            if self.input {
                                self.unit.update(UnitEV::ListenToInput)
                            } else {
                                self.unit.update(UnitEV::IgnoreInput)
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            let mut count = self.nodes.len();
                            ui.label("Nodes count");
                            let input = ui.add(egui::Slider::new(&mut count, 1..=10));
                            if input.changed() {
                                for (n, b) in self.nodes.drain(0..) {
                                    self.world.despawn(n).expect("despawn node");
                                    self.world.despawn(b).expect("despawn control");
                                }

                                let (nodes, config) = make_nodes(&mut self.world, count, self.f0);

                                self.unit.update(UnitEV::Configure(config));
                                self.nodes.extend(nodes);
                            }
                        });
                        ui.vertical(|ui| {
                            ui.label("f0");
                            let count = self.nodes.len();
                            let input =
                                ui.add(egui::Slider::new(&mut self.f0, 6.0..=1_000.0).suffix("hz"));
                            if input.changed() {
                                for (n, b) in self.nodes.drain(0..) {
                                    self.world.despawn(n).expect("despawn node");
                                    self.world.despawn(b).expect("despawn control");
                                }

                                let (nodes, config) = make_nodes(&mut self.world, count, self.f0);

                                self.unit.update(UnitEV::Configure(config));
                                self.nodes.extend(nodes);
                            }
                        });
                    });

                    for (i, node) in self
                        .nodes
                        .iter()
                        .map(|(e, _)| self.world.get::<&Node>(*e).ok())
                        .flatten()
                        .enumerate()
                    {
                        ui.label(format!("Node config {}", i + 1).as_str());

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.vertical(|ui| {
                                    ui.label("Node emit.0");
                                    let mut emit = node.f_emit.0.value();
                                    let input = ui.add(
                                        egui::Slider::new(&mut emit, 1.0..=22_000.0).suffix("hz"),
                                    );
                                    if input.changed() {
                                        node.f_emit.0.set(emit);
                                    }
                                });
                                ui.vertical(|ui| {
                                    ui.label("Node emit.1");
                                    let mut emit = node.f_emit.1.value();
                                    let input = ui.add(
                                        egui::Slider::new(&mut emit, 1.0..=22_000.0).suffix("hz"),
                                    );
                                    if input.changed() {
                                        node.f_emit.1.set(emit);
                                    }
                                });
                            });
                            ui.vertical(|ui| {
                                ui.vertical(|ui| {
                                    ui.label("Node sense.0.0");
                                    let mut emit = node.f_sense.0 .0.value();
                                    let input = ui.add(
                                        egui::Slider::new(&mut emit, MIN_F..=MAX_F).suffix("hz"),
                                    );
                                    if input.changed() {
                                        node.f_sense.0 .0.set(emit);
                                    }
                                });
                                ui.vertical(|ui| {
                                    ui.label("Node sense.0.1");
                                    let mut emit = node.f_sense.0 .1.value();
                                    let input = ui.add(
                                        egui::Slider::new(&mut emit, MIN_F..=MAX_F).suffix("hz"),
                                    );
                                    if input.changed() {
                                        node.f_sense.0 .1.set(emit);
                                    }
                                });
                            });
                            ui.vertical(|ui| {
                                ui.vertical(|ui| {
                                    ui.label("Node sense.1.0");
                                    let mut emit = node.f_sense.1 .0.value();
                                    let input = ui.add(egui::Slider::new(&mut emit, 0.01..=100.0));
                                    if input.changed() {
                                        node.f_sense.1 .0.set(emit);
                                    }
                                });
                                ui.vertical(|ui| {
                                    ui.label("Node sense.1.0");
                                    let mut emit = node.f_sense.1 .1.value();
                                    let input = ui.add(egui::Slider::new(&mut emit, 0.01..=100.0));
                                    if input.changed() {
                                        node.f_sense.1 .1.set(emit);
                                    }
                                });
                            });
                            ui.vertical(|ui| {
                                ui.label("Node pan");
                                let mut emit = node.pan.value();
                                let input =
                                    ui.add(egui::Slider::new(&mut emit, -1.0..=1.0).suffix("pan"));
                                if input.changed() {
                                    node.pan.set(emit);
                                }
                            });
                        });
                    }
                });

            // Draw oscilloscope.

            ui.horizontal(|ui| {
                let desired_size = (ui.available_width() / 2.0) * vec2(1.0, 0.25);

                ui.vertical(|ui| {
                    ui.label("Output");
                    egui::containers::Frame::canvas(ui.style()).show(ui, |ui| {
                        ui.ctx().request_repaint();

                        let snoops = self.snoops_cons.pop().unwrap_or(self.last_snoops.clone());

                        self.last_snoops = snoops.clone();

                        let thickness: f32 = 1.0;
                        let (_id, rect) = ui.allocate_space(desired_size);
                        let spots_c = self.nodes.len();
                        let to_screen_spots = emath::RectTransform::from_to(
                            Rect::from_x_y_ranges(0.0..=spots_c as f32, 0.0..=1.0),
                            rect,
                        );
                        for (i, (e, data)) in snoops.iter().enumerate() {
                            let node = self
                                .nodes
                                .iter()
                                .find(|(_, b)| b == e)
                                .map(|(n, _)| self.world.get::<&Node>(*n).unwrap());

                            let points = data.len();
                            let color = Color32::from_rgb(
                                (10 * i).try_into().unwrap(),
                                if node.is_some() { 200 } else { 50 },
                                node.as_ref()
                                    .map_or(50, |n| (125.0 + n.pan.value() * 125.0) as u8),
                            );
                            let to_screen = emath::RectTransform::from_to(
                                Rect::from_x_y_ranges(0.0..=points as f32, -1.0..=1.0),
                                rect,
                            );
                            let pts: Vec<Pos2> = data
                                .iter()
                                .enumerate()
                                .map(|(i, y)| {
                                    to_screen * pos2((points - i) as f32, softsign(y * 10.0) as f32)
                                })
                                .collect();
                            let line = epaint::Shape::line(pts, Stroke::new(thickness, color));
                            ui.painter().add(line);

                            if let Some(node) = node {
                                let pos = to_screen_spots
                                    * pos2((spots_c - i) as f32 - 0.5, softsign(2.5));
                                let spot = if node.control.value() > 0.0 {
                                    epaint::Shape::circle_filled(pos, 5.0, color)
                                } else {
                                    epaint::Shape::circle_stroke(pos, 5.0, Stroke::new(1.0, color))
                                };

                                ui.painter().add(spot);
                            }
                        }
                    });
                });

                ui.vertical(|ui| {
                    ui.label("Input spectrum");
                    egui::containers::Frame::canvas(ui.style()).show(ui, |ui| {
                        ui.ctx().request_repaint();
                        let fft = self.fft_cons.pop().unwrap_or(self.last_fft.clone());

                        self.last_fft = fft.clone();

                        let points = fft.len();
                        let color = Color32::from_rgb(250, 200, 120);
                        let thickness: f32 = 1.0;

                        let (_id, rect) = ui.allocate_space(desired_size);
                        let to_screen = emath::RectTransform::from_to(
                            Rect::from_x_y_ranges(0.0..=points as f32, -10.0..=10.0),
                            rect,
                        );

                        let pts: Vec<Pos2> = fft
                            .iter()
                            .enumerate()
                            .map(|(i, (_, y))| {
                                to_screen * pos2((points - i) as f32, softsign(*y - 10.0))
                            })
                            .collect();

                        let line = epaint::Shape::line(pts, Stroke::new(thickness, color));
                        ui.painter().add(line);
                    });
                })
            });

            if !self.input {
                for (i, k) in KEYS.iter().enumerate() {
                    if let Some((_, b)) = self.nodes.get(i) {
                        if ctx.input(|c| c.key_down(*k)) {
                            if !self.pressed.contains(b) {
                                self.unit.update(UnitEV::SetControl(*b, 1.0));
                                self.pressed.insert(*b);
                                log::info!("activated node {}", i + 1);
                            }
                        } else {
                            if self.pressed.contains(b) {
                                self.unit.update(UnitEV::SetControl(*b, 0.0));
                                self.pressed.remove(b);
                                log::info!("deactivated node {}", i + 1);
                            }
                        }
                    } else if ctx.input(|c| c.key_down(*k)) {
                        log::warn!("node is not created")
                    }
                }
            }
        });
    }
}
