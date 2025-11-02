mod context;
mod editor;
mod sensor_handles;
mod spectrum;

use common::tuner::{SpectrumData, UpdateSensorPayload};
use leptos::callback::Callback;
use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::util::raf_fn_fps::use_raf_fn_with_fps;

pub use context::{expect_tuner_service, provide_tuner_service};
pub use sensor_handles::SensorHandles;
pub use spectrum::SpectrumVisualizer;

/// Main tuner component with spectrum visualization and sensor configuration
#[component]
pub fn Tuner(#[prop(into, optional)] editor: Signal<bool>) -> impl IntoView {
    let context = expect_tuner_service();

    // Listen for spectrum data
    let UseTauriWithReturn {
        data: spectrum_data,
        error: spectrum_error,
        trigger: poll_spectrum,
    } = use_command::<SpectrumData>(common::commands::tuner::SPECTRUM_DATA);

    // Update sensor command
    let UseTauriReturn {
        error: update_sensor_error,
        trigger: update_sensor_invoke,
        ..
    } = use_invoke::<UpdateSensorPayload, (), ()>(common::commands::tuner::UPDATE_SENSOR);

    // Log errors
    Effect::new(move |_| {
        if let Some(err) = spectrum_error() {
            log::error!("Error listening to spectrum data: {err}");
        }
        if let Some(err) = update_sensor_error() {
            log::error!("Error updating sensor: {err}");
        }
    });

    // Callback for updating sensors
    let on_update_sensor = Callback::new(move |payload: UpdateSensorPayload| {
        log::debug!("Updating sensor: {:#?}", payload);
        // optimistic update
        context.config.update(|cfg| {
            if let Some(sensor) = cfg
                .as_mut()
                .and_then(|cfg| cfg.sensor_data.iter_mut().find(|s| s.key == payload.key))
            {
                sensor.min_frequency = payload.min_frequency;
                sensor.max_frequency = payload.max_frequency;
                sensor.min_magnitude = payload.min_magnitude;
                sensor.max_magnitude = payload.max_magnitude;
            }
        });
        // invoke update
        update_sensor_invoke(Some((payload, ())));
    });

    _ = use_raf_fn_with_fps(
        move |_| {
            poll_spectrum(Some(()));
            if let Some(data) = spectrum_data() {
                context.spectrum.set(Some(data));
            }
        },
        20.0,
    );

    view! {
        <div
            class="relative overflow-hidden"
            style:width=move || format!("{}px", context.layout.get().unwrap_or_default().space.x)
            style:height=move || format!("{}px", context.layout.get().unwrap_or_default().space.y)
        >
            <SpectrumVisualizer spectrum=context.spectrum layout=context.layout />

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
