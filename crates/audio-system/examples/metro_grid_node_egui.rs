use std::sync::Arc;
use std::time::Instant;

use audio_system::{
    egui_testbed::RuntimeTestbed,
    system::{
        grid::Grid,
        node::{Node, NumNodeInputs},
    },
};
use common::{
    config::{Node as NodeConfig, NodeKey},
    egui_helpers::{run_native_app, show_scrolled_left_panel_inside, show_validation_status},
};
use eframe::egui;
use enum2egui::GuiInspect;
use fundsp::{prelude::*, typenum::Unsigned};
use num_rational::Rational32;
use parking_lot::Mutex;

const WINDOW_TITLE: &str = "Metro Grid + Node demo";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];

fn main() -> eframe::Result<()> {
    run_native_app(
        WINDOW_TITLE,
        WINDOW_SIZE,
        Some("audio_system=debug,metro_grid_node_egui=debug"),
        |_cc| {
            let node_cfg = NodeConfig {
                key: NodeKey {
                    key: 0,
                    band_key: 0,
                },
                base_frequency_hz: 300.0,
                path_spacing_hz: 3.0,
                num_modes: 8,
                mode_spacing_hz: 10.0,
                mode_decay_s: 0.2,
            };

            let mut app = RuntimeDemoApp {
                testbed: RuntimeTestbed::default(),
                node_cfg: Arc::new(Mutex::new(node_cfg)),
                node_rt: Arc::new(Mutex::new(Node::new(node_cfg))),
                grid: Arc::new(Mutex::new(Grid::new(240.0_f32, &[node_cfg.key]))),
                node_key: node_cfg.key,
                bpm: 240.0,
                time_num: 1,
                time_den: 3,
                duration_num: 1,
                duration_den: 6,
                repeat_enabled: true,
                repeat_count: 2,
                event_value: 1.0,
                trigger_seq: 0,
                last_status_error: None,
            };

            let node_cfg = app.node_cfg.clone();
            let node_rt = app.node_rt.clone();
            let grid = app.grid.clone();
            let node_key = app.node_key;
            let initial_bpm = app.bpm;

            app.testbed.set_startup_network_builder(move |sample_rate| {
                *node_rt.lock() = Node::new(*node_cfg.lock());
                *grid.lock() = Grid::new(initial_bpm, &[node_key]);

                let mut net = Net::new(1, 2);
                let input_id = net.push(Box::new(sink()));
                let grid_id = net.push(Box::new(grid.lock().backend()));
                let node_id = net.push(Box::new(node_rt.lock().backend()));
                let split_id = net.push(Box::new(split::<U2>()));

                for output_index in 0..NumNodeInputs::USIZE {
                    net.connect(grid_id, output_index, node_id, output_index);
                }

                net.pipe_all(node_id, split_id);
                net.connect_input(0, input_id, 0);
                net.connect_output(split_id, 0, 0);
                net.connect_output(split_id, 1, 1);
                net.set_sample_rate(sample_rate);
                net.check();
                net
            });

            app
        },
    )
}

struct RuntimeDemoApp {
    testbed: RuntimeTestbed,
    node_cfg: Arc<Mutex<NodeConfig>>,
    node_rt: Arc<Mutex<Node<f32>>>,
    grid: Arc<Mutex<Grid>>,
    node_key: NodeKey,
    bpm: f32,
    time_num: i32,
    time_den: i32,
    duration_num: i32,
    duration_den: i32,
    repeat_enabled: bool,
    repeat_count: u32,
    event_value: f64,
    trigger_seq: u64,
    last_status_error: Option<String>,
}

impl RuntimeDemoApp {
    fn validate_ratio(num: i32, den: i32, label: &str) -> Result<Rational32, String> {
        if den <= 0 {
            return Err(format!("{label} denominator must be > 0"));
        }
        if num < 0 {
            return Err(format!("{label} numerator must be >= 0"));
        }

        Ok(Rational32::new(num, den))
    }

    fn sync_bpm(&mut self) {
        self.grid.lock().set_bpm(self.bpm.max(1.0));
    }

    fn send_event(&mut self) {
        let time = match Self::validate_ratio(self.time_num, self.time_den, "Time") {
            Ok(value) => value,
            Err(err) => {
                self.last_status_error = Some(err);
                return;
            }
        };

        let duration = match Self::validate_ratio(self.duration_num, self.duration_den, "Duration")
        {
            Ok(value) => value,
            Err(err) => {
                self.last_status_error = Some(err);
                return;
            }
        };

        let repeat = if self.repeat_enabled {
            Some(self.repeat_count)
        } else {
            None
        };

        let result = self.grid.lock().schedule_event(
            &self.node_key,
            time,
            duration,
            repeat,
            self.event_value,
        );

        match result {
            Ok(()) => {
                self.trigger_seq = self.trigger_seq.saturating_add(1);
                self.testbed
                    .register_visual_trigger(self.trigger_seq, Instant::now());
                self.last_status_error = None;
            }
            Err(err) => {
                self.last_status_error = Some(format!("Send event failed: {err}"));
            }
        }
    }

    fn render_scheduler_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Metro grid scheduling");
        ui.small(format!(
            "Scheduling key: ({}, {})",
            self.node_key.key, self.node_key.band_key
        ));

        ui.horizontal(|ui| {
            ui.label("BPM");
            let response = ui.add(
                egui::DragValue::new(&mut self.bpm)
                    .speed(0.5)
                    .range(1.0..=400.0),
            );
            if response.changed() {
                self.sync_bpm();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Time");
            ui.add(
                egui::DragValue::new(&mut self.time_num)
                    .speed(1.0)
                    .range(0..=128),
            );
            ui.label("/");
            ui.add(
                egui::DragValue::new(&mut self.time_den)
                    .speed(1.0)
                    .range(1..=128),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Duration");
            ui.add(
                egui::DragValue::new(&mut self.duration_num)
                    .speed(1.0)
                    .range(0..=128),
            );
            ui.label("/");
            ui.add(
                egui::DragValue::new(&mut self.duration_den)
                    .speed(1.0)
                    .range(1..=128),
            );
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.repeat_enabled, "Repeat");
            ui.add_enabled(
                self.repeat_enabled,
                egui::DragValue::new(&mut self.repeat_count)
                    .speed(1.0)
                    .range(0..=64),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Event amplitude");
            ui.add(
                egui::DragValue::new(&mut self.event_value)
                    .speed(0.01)
                    .range(0.0..=4.0),
            );
        });

        let send_clicked = ui.button("Send Event").clicked();
        let send_key = ui.ctx().input(|input| input.key_pressed(egui::Key::Space));
        if send_clicked || send_key {
            self.send_event();
        }

        show_validation_status(
            ui,
            self.last_status_error.as_deref(),
            "Event queued successfully",
        );
    }
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        show_scrolled_left_panel_inside(ui, "runtime_controls", 360.0, None, |ui| {
            self.node_cfg.lock().ui_mut(ui);

            ui.separator();
            self.render_scheduler_ui(ui);

            ui.separator();
            self.testbed.render_controls(ui);
            self.testbed.render_runtime_messages(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.testbed.render_diagnostics(ui);
        });
    }
}
