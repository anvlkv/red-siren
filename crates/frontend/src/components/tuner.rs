mod context;
mod editor;
mod sensor_handles;
mod spectrum;

use common::tuner::{Config, UpdateSensorPayload};
use leptos::callback::Callback;
use leptos::prelude::*;
use tauri_use::{use_invoke, UseTauriReturn};

pub use context::{expect_tuner_service, provide_tuner_service};
pub use sensor_handles::SensorHandles;
pub use spectrum::SpectrumVisualizer;

/// Main tuner component with spectrum visualization and sensor configuration
#[component]
pub fn Tuner(#[prop(into, optional)] editor: Signal<bool>) -> impl IntoView {
    let context = expect_tuner_service();

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
            context.config.set(Some(data));
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
            style:width=move || format!("{}px", context.layout.get().unwrap_or_default().space.x)
            style:height=move || format!("{}px", context.layout.get().unwrap_or_default().space.y)
        >
            <SpectrumVisualizer layout=context.layout />

            <SensorHandles
                config=Signal::derive(move || context.config.get().unwrap_or_default())
                layout=Signal::derive(move || context.layout.get().unwrap_or_default())
                on_update=on_update_sensor
            />
            <Show when=move || editor()>
                <editor::EditorOverlay />
            </Show>
        </div>
    }
}
