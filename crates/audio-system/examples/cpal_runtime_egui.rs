use audio_system::egui_testbed::RuntimeTestbed;
use common::egui_helpers::{run_native_app, show_scrolled_left_panel_inside};
use eframe::egui;

const WINDOW_TITLE: &str = "CPAL runtime gate";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];

#[derive(Default)]
struct RuntimeDemoApp {
    testbed: RuntimeTestbed,
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        show_scrolled_left_panel_inside(ui, "runtime_controls", 360.0, None, |ui| {
            self.testbed.render_controls(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| self.testbed.render_diagnostics(ui));
    }
}

fn main() -> eframe::Result<()> {
    run_native_app(WINDOW_TITLE, WINDOW_SIZE, |_cc| RuntimeDemoApp::default())
}
