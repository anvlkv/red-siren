use eframe::egui;
use std::sync::Once;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Idle,
    Success,
    Warning,
    Error,
    Info,
}

impl StatusTone {
    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Idle => egui::Color32::from_rgb(160, 160, 160),
            Self::Success => egui::Color32::from_rgb(102, 204, 122),
            Self::Warning => egui::Color32::from_rgb(255, 196, 92),
            Self::Error => egui::Color32::from_rgb(255, 120, 120),
            Self::Info => egui::Color32::from_rgb(120, 195, 255),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraControls {
    pub pitch: f64,
    pub yaw: f64,
    pub zoom: Option<f64>,
    pub pan_x: Option<f32>,
    pub pan_y: Option<f32>,
}

impl CameraControls {
    pub fn orbit(pitch: f64, yaw: f64) -> Self {
        Self {
            pitch,
            yaw,
            zoom: None,
            pan_x: None,
            pan_y: None,
        }
    }

    pub fn orbit_zoom_pan(pitch: f64, yaw: f64, zoom: f64, pan_x: f32, pan_y: f32) -> Self {
        Self {
            pitch,
            yaw,
            zoom: Some(zoom),
            pan_x: Some(pan_x),
            pan_y: Some(pan_y),
        }
    }

    pub fn show_collapsing(
        &mut self,
        ui: &mut egui::Ui,
        heading: &str,
        id_salt: impl std::hash::Hash,
    ) {
        egui::CollapsingHeader::new(heading)
            .id_salt(id_salt)
            .default_open(false)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(
                        &mut self.pitch,
                        -std::f64::consts::PI..=std::f64::consts::PI,
                    )
                    .text("Pitch"),
                );
                ui.add(
                    egui::Slider::new(&mut self.yaw, -std::f64::consts::PI..=std::f64::consts::PI)
                        .text("Yaw"),
                );
                if let Some(zoom) = &mut self.zoom {
                    ui.add(egui::Slider::new(zoom, 0.35..=3.5).text("Zoom"));
                }
                if let Some(pan_x) = &mut self.pan_x {
                    ui.add(egui::Slider::new(pan_x, -480.0..=480.0).text("Pan X"));
                }
                if let Some(pan_y) = &mut self.pan_y {
                    ui.add(egui::Slider::new(pan_y, -360.0..=360.0).text("Pan Y"));
                }
            });
    }
}

fn init_terminal_logger() {
    static LOGGER_INIT: Once = Once::new();

    LOGGER_INIT.call_once(|| {
        let mut builder =
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(concat!(
                "warn,",
                env!("CARGO_PKG_NAME"),
                "=trace"
            )));
        builder
            .format_timestamp_millis()
            .format_target(true)
            .target(env_logger::Target::Stderr);

        let _ = builder.try_init();
    });
}

pub fn run_native_app<T, F>(
    title: &'static str,
    inner_size: [f32; 2],
    app_creator: F,
) -> eframe::Result<()>
where
    T: eframe::App + 'static,
    F: FnOnce(&eframe::CreationContext<'_>) -> T + 'static,
{
    init_terminal_logger();
    log::info!(
        "starting native app title={title} inner_size={}x{}",
        inner_size[0],
        inner_size[1]
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size(inner_size),
        ..Default::default()
    };

    eframe::run_native(
        title,
        options,
        Box::new(move |cc| Ok(Box::new(app_creator(cc)))),
    )
}

pub fn show_scrolled_left_panel_inside(
    ui: &mut egui::Ui,
    panel_id: &'static str,
    min_size: f32,
    max_size: Option<f32>,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let panel = egui::Panel::left(panel_id).min_size(min_size);
    let panel = if let Some(max_size) = max_size {
        panel.max_size(max_size)
    } else {
        panel
    };

    panel.show_inside(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| add_contents(ui));
    });
}

pub fn show_validation_status(ui: &mut egui::Ui, error: Option<&str>, success_label: &str) {
    match error {
        Some(error) => ui.colored_label(StatusTone::Error.color(), format!("✗ {error}")),
        None => ui.colored_label(StatusTone::Success.color(), format!("✓ {success_label}")),
    };
}

pub fn show_action_error_messages(
    ui: &mut egui::Ui,
    action: &Option<String>,
    error: &Option<String>,
) {
    if let Some(action) = action {
        ui.colored_label(StatusTone::Info.color(), action);
    }
    if let Some(error) = error {
        ui.colored_label(StatusTone::Error.color(), error);
    }
}

pub fn enum_combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    id_salt: impl std::hash::Hash,
    current: &mut T,
    options: &[(T, &'static str)],
) {
    let selected = options
        .iter()
        .find_map(|(value, name)| (*value == *current).then_some(*name))
        .unwrap_or("Select");

    ui.label(label);
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (value, name) in options {
                ui.selectable_value(current, *value, *name);
            }
        });
}
