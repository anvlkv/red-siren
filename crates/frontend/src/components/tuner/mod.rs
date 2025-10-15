//! Tuner component for spectrum visualization and sensor configuration

mod context;
mod sensor_handles;
mod spectrum;

use leptos::callback::Callback;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use tauri_use::{use_invoke, use_listen, UseListenReturn, UseTauriReturn};

use crate::util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn};

use common::tuner::{Config, Layout as TunerLayout, SpectrumData, UpdateSensorPayload};
use common::RouteId;

pub use context::{expect_tuner_service, provide_tuner_service, TunerService};
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
    let UseListenReturn {
        data: spectrum_data,
        error: spectrum_error,
        open: spectrum_open,
        close: close_spectrum,
        ..
    } = use_listen::<SpectrumData>(tauri_use::EventType::Custom(
        common::events::tuner::SPECTRUM_DATA,
    ));

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

    // Update context with spectrum data
    Effect::new(move |_| {
        if let Some(data) = spectrum_data() {
            context.spectrum.set(Some(data));
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
        spectrum_open();
    });

    // Cleanup listeners on unmount (stream stop handled at page level)
    on_cleanup(move || {
        close_config_update();
        close_spectrum();
    });

    // Callback for updating sensors
    let on_update_sensor = Callback::new(move |payload: UpdateSensorPayload| {
        log::debug!("Updating sensor {}: {:?}", payload.index, payload);
        update_sensor_invoke(Some((payload, ())));
    });

    // Callback for selecting a sensor
    let on_select_sensor = Callback::new(move |index: Option<usize>| {
        context.active_sensor.set(index);
    });

    // Derived signals for visualization
    let config_signal = Signal::derive(move || context.config.get());
    let layout_signal: Signal<Option<TunerLayout>> = Signal::derive(move || tuner_layout());
    let spectrum_signal = Signal::derive(move || context.spectrum.get());
    let active_sensor_signal = Signal::derive(move || context.active_sensor.get());

    view! {
        <div class="relative w-full h-full overflow-hidden">
            // Spectrum visualization layer
            <SpectrumVisualizer spectrum=spectrum_signal layout=layout_signal />

            // Sensor handles layer
            <SensorHandles
                config=config_signal
                layout=Signal::derive(move || tuner_layout())
                on_update=on_update_sensor
                active_sensor=active_sensor_signal
                on_select=on_select_sensor
            />

            // Info overlay (optional - shows current active sensor)
            <Show when=move || active_sensor_signal.get().is_some()>
                <div class="absolute top-4 left-4 p-2 bg-white/80 dark:bg-black/80 rounded-md">
                    <span class="text-xs text-gray dark:text-cinnabar">
                        "Sensor "
                        {move || {
                            active_sensor_signal.get().map(|i| i.to_string()).unwrap_or_default()
                        }}
                    </span>
                </div>
            </Show>
        </div>
    }
}
