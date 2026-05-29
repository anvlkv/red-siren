use audio_system::egui_testbed::RuntimeTestbed;
use eframe::egui;

const WINDOW_TITLE: &str = "CPAL runtime gate";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];

#[derive(Default)]
struct RuntimeDemoApp {
    testbed: RuntimeTestbed,
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("runtime_controls")
            .min_size(360.0)
            .show_inside(ui, |ui| self.testbed.render_controls(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| self.testbed.render_diagnostics(ui));
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size(WINDOW_SIZE),
        ..Default::default()
    };

    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|_cc| Ok(Box::new(RuntimeDemoApp::default()))),
    )
}
