use std::sync::mpsc::{sync_channel, Receiver};

use au_core::{Node, Unit, UnitEV, MAX_F, MIN_F};
use eframe::egui::{self, *};
use fundsp::hacker32::*;
use hecs::Entity;

struct State {
    unit: Unit,
    input: bool,
    node: Node,
    last_fft: Vec<(f32, f32)>,
    last_snoops: Vec<(Entity, Vec<f32>)>,
    fft_receiver: Receiver<Vec<(f32, f32)>>,
    snoop_receiver: Receiver<Vec<(Entity, Vec<f32>)>>,
}

fn main() {
    simple_logger::init_with_level(log::Level::Info).expect("couldn't initialize logging");

    let unit = Unit::new();
    run(unit).unwrap();
}

fn run(mut unit: Unit) -> Result<(), anyhow::Error> {
    let node = Node {
        f_base: shared(440.0),
        f_emit: (shared(440.0), shared(480.0)),
        f_sense: ((shared(MIN_F), shared(MAX_F)), (shared(0.0), shared(1.0))),
        control: shared(0.0),
        button: Entity::DANGLING,
        pan: shared(0.0),
    };

    
    let (fft_sender, fft_receiver) = sync_channel(64);
    let (snoop_sender, snoop_receiver) = sync_channel(64);
    
    unit.run(fft_sender, snoop_sender)?;
    
    unit.update(UnitEV::Configure(vec![node.clone()]));
    

    let state = State {
        unit,
        node,
        fft_receiver,
        snoop_receiver,
        input: true,
        last_fft: vec![],
        last_snoops: vec![],
    };
    let viewport = ViewportBuilder::default().with_min_inner_size(vec2(360.0, 480.0));

    let options = eframe::NativeOptions {
        viewport,
        ..eframe::NativeOptions::default()
    };

    eframe::run_native(
        "Single node example",
        options,
        Box::new(|_cc| Box::new(state)),
    )
    .unwrap();

    Ok(())
}

impl eframe::App for State {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Single node example");
            ui.separator();
            ui.end_row();

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

            ui.label("Node");
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Node emit.0");
                        let mut emit = self.node.f_emit.0.value();
                        let input =
                            ui.add(egui::Slider::new(&mut emit, 1.0..=22_000.0).suffix("hz"));
                        if input.changed() {
                            self.node.f_emit.0.set(emit);
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label("Node emit.1");
                        let mut emit = self.node.f_emit.1.value();
                        let input =
                            ui.add(egui::Slider::new(&mut emit, 1.0..=22_000.0).suffix("hz"));
                        if input.changed() {
                            self.node.f_emit.1.set(emit);
                        }
                    });
                });
                ui.vertical(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Node sense.0.0");
                        let mut emit = self.node.f_sense.0 .0.value();
                        let input =
                            ui.add(egui::Slider::new(&mut emit, MIN_F..=MAX_F).suffix("hz"));
                        if input.changed() {
                            self.node.f_sense.0 .0.set(emit);
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label("Node sense.0.1");
                        let mut emit = self.node.f_sense.0 .1.value();
                        let input =
                            ui.add(egui::Slider::new(&mut emit, MIN_F..=MAX_F).suffix("hz"));
                        if input.changed() {
                            self.node.f_sense.0 .1.set(emit);
                        }
                    });
                });
                ui.vertical(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Node sense.1.0");
                        let mut emit = self.node.f_sense.1 .0.value();
                        let input = ui.add(egui::Slider::new(&mut emit, 0.0..=1.0));
                        if input.changed() {
                            self.node.f_sense.1 .0.set(emit);
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label("Node sense.1.0");
                        let mut emit = self.node.f_sense.1 .1.value();
                        let input = ui.add(egui::Slider::new(&mut emit, 0.0..=1.0));
                        if input.changed() {
                            self.node.f_sense.1 .1.set(emit);
                        }
                    });
                });
                ui.vertical(|ui| {
                    ui.label("Node pan");
                    let mut emit = self.node.pan.value();
                    let input = ui.add(egui::Slider::new(&mut emit, -1.0..=1.0).suffix("pan"));
                    if input.changed() {
                        self.node.pan.set(emit);
                    }
                });
            });

            ui.label("Oscilloscope");
            egui::containers::Frame::canvas(ui.style()).show(ui, |ui| {
                ui.ctx().request_repaint();

                let snoops = self
                    .snoop_receiver
                    .try_recv()
                    .unwrap_or(self.last_snoops.clone());

                self.last_snoops = snoops.clone();

                let thickness: f32 = 1.0;
                let desired_size = ui.available_width() * vec2(1.0, 0.25);
                let (_id, rect) = ui.allocate_space(desired_size);

                for (i, (_, data)) in snoops.iter().enumerate() {
                    let color = Color32::from_rgb((10 * i).try_into().unwrap_or(0), 200, 220);
                    let points = data.len();
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
                }
            });

            ui.label("Input spectrum");
            egui::containers::Frame::canvas(ui.style()).show(ui, |ui| {
                ui.ctx().request_repaint();
                let fft = self
                    .fft_receiver
                    .try_recv()
                    .unwrap_or(self.last_fft.clone());
                self.last_fft = fft.clone();

                let points = fft.len();
                let color = Color32::from_rgb(250, 200, 120);
                let thickness: f32 = 1.0;
                let desired_size = ui.available_width() * vec2(1.0, 0.25);
                let (_id, rect) = ui.allocate_space(desired_size);
                let to_screen = emath::RectTransform::from_to(
                    Rect::from_x_y_ranges(0.0..=points as f32, -1.0..=1.0),
                    rect,
                );

                let pts: Vec<Pos2> = fft
                    .iter()
                    .enumerate()
                    .map(|(i, (_, y))| to_screen * pos2((points - i) as f32, *y * -1.0))
                    .collect();

                let line = epaint::Shape::line(pts, Stroke::new(thickness, color));
                ui.painter().add(line);
            });

            if !self.input {
                if ctx.input(|c| c.key_down(Key::Space)) {
                    self.unit.update(UnitEV::SetControl(Entity::DANGLING, 1.0));
                } else {
                    self.unit.update(UnitEV::SetControl(Entity::DANGLING, 0.0));
                }
            }
        });
    }
}
