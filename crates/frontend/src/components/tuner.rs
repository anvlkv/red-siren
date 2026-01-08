mod context;
mod editor;
mod sensor_handles;
mod spectrum;

use common::tuner::{Config, UpdateSensorPayload};
use leptos::callback::Callback;
use leptos::prelude::*;
use leptos_use::use_device_pixel_ratio;
use tauri_use::{use_invoke, UseTauriReturn};

pub use context::{expect_tuner_service, provide_tuner_service};
pub use sensor_handles::SensorHandles;
pub use spectrum::SpectrumVisualizer;

/// Canonical canvas transform shared by all tuner canvases
#[derive(Clone, PartialEq)]
pub struct CanvasTransform {
    pub world_scale_css: f64,    // CSS px per world unit
    pub device_scale: f64,       // device px per world unit
    pub translate_x_dev: f64,    // device px offset X
    pub translate_y_dev: f64,    // device px offset Y
    pub backing_width_dev: u32,  // device px width
    pub backing_height_dev: u32, // device px height
    pub device_pixel_ratio: f64, // DPR
}

impl CanvasTransform {
    pub fn css_to_world(&self, css_x: f64, css_y: f64) -> (f64, f64) {
        let dev_x = css_x * self.device_pixel_ratio;
        let dev_y = css_y * self.device_pixel_ratio;
        let world_x = (dev_x - self.translate_x_dev) / self.device_scale;
        let world_y = (dev_y - self.translate_y_dev) / self.device_scale;
        (world_x, world_y)
    }

    pub fn css_to_device(&self, css_x: f64, css_y: f64) -> (f64, f64) {
        let dev_x = css_x * self.device_pixel_ratio;
        let dev_y = css_y * self.device_pixel_ratio;
        (dev_x, dev_y)
    }
}

/// Main tuner component with spectrum visualization and sensor configuration
#[component]
pub fn Tuner(#[prop(into, optional)] editor: Signal<bool>) -> impl IntoView {
    let context = expect_tuner_service();
    let layout_sig = context.layout;
    let config_sig = context.config;

    let pixel_ratio = use_device_pixel_ratio();

    // World scale in CSS px per world unit (container-sized: 1 CSS px == 1 world unit)
    let world_scale_css = Memo::new(move |_| 1.0);

    // Device scale per world unit
    let device_scale = Memo::new(move |_| world_scale_css() * pixel_ratio());

    // No letterboxing within container: translate is zero
    let translate_x_dev = Memo::new(move |_| 0.0);

    let translate_y_dev = Memo::new(move |_| 0.0);

    // Backing bitmap size in device pixels = layout.space * DPR
    let backing_width_dev = Memo::new(move |_| {
        let pr = pixel_ratio();
        let space_x = layout_sig.get().unwrap_or_default().space.x;
        (space_x * pr).round().clamp(1.0, f64::MAX) as u32
    });

    let backing_height_dev = Memo::new(move |_| {
        let pr = pixel_ratio();
        let space_y = layout_sig.get().unwrap_or_default().space.y;
        (space_y * pr).round().clamp(1.0, f64::MAX) as u32
    });

    // Pack canonical transform
    let canvas_transform = Memo::new(move |_| CanvasTransform {
        world_scale_css: world_scale_css(),
        device_scale: device_scale(),
        translate_x_dev: translate_x_dev(),
        translate_y_dev: translate_y_dev(),
        backing_width_dev: backing_width_dev(),
        backing_height_dev: backing_height_dev(),
        device_pixel_ratio: pixel_ratio(),
    });

    // Update sensor command
    let UseTauriReturn {
        error: update_sensor_error,
        trigger: update_sensor_invoke,
        data: update_sensor_data,
        ..
    } = use_invoke::<UpdateSensorPayload, (), Config>(common::commands::tuner::UPDATE_SENSOR);

    // Log errors
    Effect::new(move |_| {
        if let Some(err) = update_sensor_error() {
            log::error!("Error updating sensor: {err}");
        }
    });

    Effect::new(move |_| {
        if let Some(data) = update_sensor_data() {
            config_sig.set(Some(data));
        }
    });

    // Callback for updating sensors
    let on_update_sensor = Callback::new(move |payload: UpdateSensorPayload| {
        log::debug!("Updating sensor: {:#?}", payload);
        // invoke update
        update_sensor_invoke(Some((payload, ())));
    });

    view! {
        <div
            class="relative overflow-hidden"
            style:width=move || format!("{}px", layout_sig.get().unwrap_or_default().space.x)
            style:height=move || format!("{}px", layout_sig.get().unwrap_or_default().space.y)
        >
            <SpectrumVisualizer layout=layout_sig canvas_transform=canvas_transform />

            <SensorHandles
                config=Signal::derive(move || config_sig.get().unwrap_or_default())
                layout=Signal::derive(move || layout_sig.get().unwrap_or_default())
                canvas_transform=canvas_transform
                on_update=on_update_sensor
            />
            <Show when=move || editor()>
                <editor::EditorOverlay />
            </Show>
        </div>
    }
}
