pub mod app;
pub mod device;
pub mod visualization;

pub use app::{
    enum_combo, run_native_app, show_action_error_messages, show_frequency_spectrum_chart,
    show_scrolled_left_panel_inside, show_validation_status, CameraControls, StatusTone,
};
pub use device::{device_key, find_selected_device, selected_device_label, show_device_combo};
pub use visualization::{show_buffer_line_chart, VisualizationThrottle};

pub fn show_frequency_peaks(ui: &mut egui::Ui, peaks: &[(u32, f32)], empty_label: &str) {
    if peaks.is_empty() {
        ui.small(empty_label);
        return;
    }

    for (frequency, amplitude) in peaks {
        ui.monospace(format!("{frequency:>5} Hz  {amplitude:.6e}"));
    }
}
