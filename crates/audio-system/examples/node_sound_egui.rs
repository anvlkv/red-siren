use std::sync::Arc;

use audio_system::egui_testbed::RuntimeTestbed;
use common::egui_helpers::{run_native_app, show_scrolled_left_panel_inside};
use eframe::egui;
use enum2egui::GuiInspect;
use fundsp::prelude::*;
use parking_lot::Mutex;

const WINDOW_TITLE: &str = "Node demo";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];

fn main() -> eframe::Result<()> {
    run_native_app(
        WINDOW_TITLE,
        WINDOW_SIZE,
        Some("audio_system=debug,node_sound_egui=debug"),
        |_cc| {
            let node_cfg = common::config::Node {
                key: common::config::NodeKey {
                    key: 0,
                    band_key: 0,
                },
                base_frequency_hz: 300.0,
                path_spacing_hz: 3.0,
                num_modes: 8,
                mode_spacing_hz: 10.0,
                mode_decay_s: 8.0,
            };
            let mut app = RuntimeDemoApp {
                testbed: RuntimeTestbed::default(),
                node: Arc::new(Mutex::new(node_cfg)),
                node_rt: Arc::new(Mutex::new(audio_system::system::node::Node::new(node_cfg))),
                xct_1: true,
                xct_2: false,
                xct_3: false,
                sh_xct_1: Arc::new(Shared::new(0.0)),
                sh_xct_2: Arc::new(Shared::new(0.0)),
                sh_xct_3: Arc::new(Shared::new(0.0)),
            };

            let node_rt = app.node_rt.clone();
            let node_cfg = app.node.clone();
            let sh_xct_1 = app.sh_xct_1.clone();
            let sh_xct_2 = app.sh_xct_2.clone();
            let sh_xct_3 = app.sh_xct_3.clone();

            app.testbed.set_startup_network_builder(move |sample_rate| {
                *node_rt.lock() = audio_system::system::node::Node::new(*node_cfg.lock());

                let mut net = Net::new(1, 2);
                let input_id = net.push(Box::new(sink()));
                let node_id = net.push(Box::new(node_rt.lock().backend()));
                let split_id = net.push(Box::new(split::<U2>()));

                let backend_id = net.push(Box::new(
                    var(&sh_xct_1) >> dcblock_hz::<f32>(50.0)
                        | var(&sh_xct_2) >> dcblock_hz::<f32>(50.0)
                        | var(&sh_xct_3) >> dcblock_hz::<f32>(50.0),
                ));

                net.pipe_all(backend_id, node_id);
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
    node: Arc<Mutex<common::config::Node>>,
    node_rt: Arc<Mutex<audio_system::system::node::Node<f32>>>,
    xct_1: bool,
    xct_2: bool,
    xct_3: bool,
    sh_xct_1: Arc<Shared>,
    sh_xct_2: Arc<Shared>,
    sh_xct_3: Arc<Shared>,
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        show_scrolled_left_panel_inside(ui, "runtime_controls", 360.0, None, |ui| {
            self.node.lock().ui_mut(ui);
            let node_rt = self.node_rt.clone();
            ui.add(
                egui::widgets::DragValue::from_get_set(move |val| {
                    let lock = node_rt.lock();
                    let shared = &lock.path_spread_coeff;
                    if let Some(val) = val {
                        shared.set_value(val as f32);
                    }
                    shared.value() as f64
                })
                .range(-1.0..=1.0),
            );
            let node_rt = self.node_rt.clone();
            ui.add(
                egui::widgets::DragValue::from_get_set(move |val| {
                    let lock = node_rt.lock();
                    let shared = &lock.mode_spacing_coeff;
                    if let Some(val) = val {
                        shared.set_value(val as f32);
                    }
                    shared.value() as f64
                })
                .range(-1.0..=1.0),
            );
            self.testbed.render_controls(ui);
            ui.separator();
            ui.checkbox(&mut self.xct_1, "Excite first path");
            ui.checkbox(&mut self.xct_2, "Excite second path");
            ui.checkbox(&mut self.xct_3, "Excite third path");
            let response = ui.add(egui::Button::new("Excite (hold mouse or Space)"));
            let pointer_held = response.is_pointer_button_down_on();
            let key_held = ui.ctx().input(|input| input.key_down(egui::Key::Space));

            if pointer_held || key_held {
                if self.xct_1 {
                    self.sh_xct_1.set_value(1.0);
                }
                if self.xct_2 {
                    self.sh_xct_2.set_value(1.0);
                }
                if self.xct_3 {
                    self.sh_xct_3.set_value(1.0);
                }
            } else {
                self.sh_xct_1.set_value(0.0);
                self.sh_xct_2.set_value(0.0);
                self.sh_xct_3.set_value(0.0);
            }
        });
        egui::CentralPanel::default().show_inside(ui, |ui| self.testbed.render_diagnostics(ui));
    }
}
