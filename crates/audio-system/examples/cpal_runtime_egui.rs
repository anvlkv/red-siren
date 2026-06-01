use std::sync::Arc;

use audio_system::egui_testbed::RuntimeTestbed;
use common::egui_helpers::{run_native_app, show_scrolled_left_panel_inside};
use eframe::egui;
use fundsp::prelude::*;
use parking_lot::Mutex;

const WINDOW_TITLE: &str = "CPAL runtime demo";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];

fn main() -> eframe::Result<()> {
    run_native_app(
        WINDOW_TITLE,
        WINDOW_SIZE,
        Some("audio_system=trace,cpal_runtime_egui=trace"),
        |_cc| {
            let seq = sequencer::Sequencer::new(0, 1, sequencer::ReplayMode::None);
            // seq.push_relative(start_time, end_time, fade_ease, fade_in_time, fade_out_time, unit)
            let mut app = RuntimeDemoApp {
                testbed: RuntimeTestbed::default(),
                seq: Arc::new(Mutex::new(seq)),
            };

            let seq = app.seq.clone();
            app.testbed.set_startup_network_builder(move |sample_rate| {
                let mut sequencer = sequencer::Sequencer::new(0, 1, sequencer::ReplayMode::None);
                sequencer.set_sample_rate(sample_rate);
                let bak = sequencer.backend();
                *seq.lock() = sequencer;
                let mut net = Net::new(1, 2);
                let input_id = net.push(Box::new(sink()));
                // let tone_id = net.push(Box::new(sine_hz::<f32>(440.0) >> split::<U2>()));
                let tone_id = net.push(Box::new(
                    follow(0.001)
                        // >> split::<U8>()
                        >> busi::<U8, _, _>(|i| {
                            resonator_hz::<f32>(313.0 + (0.1 * i as f32), 500.0)
                        })
                        >> split::<U2>(),
                ));

                let backend_id = net.push(Box::new(bak));

                net.pipe_all(backend_id, tone_id);

                net.connect_input(0, input_id, 0);
                net.connect_output(tone_id, 0, 0);
                net.connect_output(tone_id, 1, 1);
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
    seq: Arc<Mutex<sequencer::Sequencer>>,
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        show_scrolled_left_panel_inside(ui, "runtime_controls", 360.0, None, |ui| {
            self.testbed.render_controls(ui);
            let response = ui.add(egui::Button::new("Excite (hold mouse or Space)"));
            let pointer_held = response.is_pointer_button_down_on();
            let key_held = ui.ctx().input(|input| input.key_down(egui::Key::Space));

            if pointer_held || key_held {
                self.seq.lock().push_relative(
                    0.0,
                    1.0,
                    Fade::Smooth,
                    0.0,
                    0.01,
                    Box::new(impulse::<U1>()),
                );
            }
        });
        egui::CentralPanel::default().show_inside(ui, |ui| self.testbed.render_diagnostics(ui));
    }
}
