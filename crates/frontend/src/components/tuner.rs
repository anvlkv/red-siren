mod context;
mod sensor_handles;
mod spectrum;

use leptos::callback::Callback;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use tauri_use::{
    use_command, use_invoke, use_listen, UseListenReturn, UseTauriReturn, UseTauriWithReturn,
};

use crate::util::raf_fn_fps::use_raf_fn_with_fps;
use crate::util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn};

use common::tuner::{Config, Layout as TunerLayout, SpectrumData, UpdateSensorPayload};
use common::RouteId;

pub use context::{expect_tuner_service, provide_tuner_service};
pub use sensor_handles::SensorHandles;
pub use spectrum::SpectrumVisualizer;

/// Main tuner component with spectrum visualization and sensor configuration
#[component]
pub fn Tuner() -> impl IntoView {
    let context = expect_tuner_service();

    // Get tuner config resource
    let UseTauriResourceReturn {
        data: config,
        error: config_error,
        refetch: _refetch_config,
        ..
    } = use_tauri_resource::<Config>(common::commands::tuner::CONFIG);

    // Get tuner layout resource (safe-area-aware baseline, orientation, etc.)

    // Get tuner layout resource (safe-area-aware baseline, orientation, etc.)
    let UseTauriResourceReturn {
        data: tuner_layout,
        error: tuner_layout_error,
        ..
    } = use_tauri_resource::<TunerLayout>(common::commands::tuner::LAYOUT);

    // Listen for config updates
    let UseListenReturn {
        data: config_update,
        error: config_update_error,
        open: config_update_open,
        close: close_config_update,
        ..
    } = use_listen::<Config>(tauri_use::EventType::Custom(common::events::tuner::CONFIG));

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

    // Get setup state to check mic permission
    let UseTauriResourceReturn {
        data: setup_state,
        error: setup_state_error,
        ..
    } = use_tauri_resource::<common::commands::health::SetupStatePayload>(
        common::commands::health::SETUP_STATE,
    );

    // Get navigation function
    let navigate = use_navigate();

    // Check mic permission on mount and redirect if needed
    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            if let Some(err) = setup_state_error() {
                log::error!("Error getting setup state: {err}");
            }

            if let Some(state) = setup_state() {
                if state.mic_permission.is_none() {
                    log::info!("Mic permission not set, redirecting to Permissions");
                    navigate(RouteId::Permissions.as_ref(), Default::default());
                }
            }
        }
    });

    // Update context when config changes
    Effect::new(move |_| {
        if let Some(cfg) = config() {
            context.config.set(Some(cfg));
        }
        if let Some(cfg) = config_update() {
            context.config.set(Some(cfg));
            // Don't refetch - it causes an infinite loop
        }
    });

    // Log errors
    Effect::new(move |_| {
        if let Some(err) = config_error() {
            log::error!("Error loading tuner config: {err}");
        }
        if let Some(err) = config_update_error() {
            log::error!("Error listening to config updates: {err}");
        }
        if let Some(err) = tuner_layout_error() {
            log::error!("Error loading tuner layout: {err}");
        }
        if let Some(err) = spectrum_error() {
            log::error!("Error listening to spectrum data: {err}");
        }
        if let Some(err) = update_sensor_error() {
            log::error!("Error updating sensor: {err}");
        }
    });

    // Start listening when component mounts (stream lifetime handled at page level via TunerService)
    Effect::new(move |_| {
        config_update_open();
    });

    // Cleanup listeners on unmount (stream stop handled at page level)
    on_cleanup(move || {
        close_config_update();
    });

    // Callback for updating sensors
    let on_update_sensor = Callback::new(move |payload: UpdateSensorPayload| {
        log::debug!("Updating sensor: {:#?}", payload);
        update_sensor_invoke(Some((payload, ())));
    });

    // Callback for selecting a sensor
    let on_select_sensor = Callback::new(move |index: Option<usize>| {
        context.active_sensor.set(index);
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
            style:width=move || format!("{}px", tuner_layout().unwrap_or_default().space.x)
            style:height=move || format!("{}px", tuner_layout().unwrap_or_default().space.y)
        >
            <SpectrumVisualizer spectrum=context.spectrum layout=tuner_layout />

            <SensorHandles
                config=context.config
                layout=tuner_layout
                on_update=on_update_sensor
                active_sensor=context.active_sensor
                on_select=on_select_sensor
            />
        </div>
    }
}
