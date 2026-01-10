use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use common::tuner::{
    Config, Layout, ReflectTunerConstraints, SpectrumData, UpdateNyThresholdPayload,
    UpdateRangePayload, UpdateWetRatioPayload,
};

use crate::util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn};

#[derive(Clone)]
pub struct TunerService {
    /// Current tuner configuration (populated by listener/resource logic outside service)
    pub config: RwSignal<Option<Config>>,
    pub layout: RwSignal<Option<Layout>>,
    /// Latest spectrum snapshot
    pub spectrum: RwSignal<Option<SpectrumData>>,
    /// Reset tuner configuration to defaults (backend: `tuner_reset_config`)
    pub reset: Callback<()>,
    /// Start tuner input / spectrum stream (backend: `tuner_start_stream`)
    pub start_stream: Callback<()>,
    /// Stop tuner input / spectrum stream (backend: `tuner_stop_stream`)
    pub stop_stream: Callback<()>,
    /// Toggle tuner probe stream
    pub probe: Callback<()>,
    /// Whether tuner audio probe is active
    pub probe_active: Signal<bool>,
    /// Tuner constraints (populated by listener/resource logic outside service)
    pub constraints: Memo<ReflectTunerConstraints>,
    /// Update spectrum range
    pub update_spectrum_range: Callback<UpdateRangePayload>,
    /// Update NY compression threshold
    pub update_ny_threshold: Callback<f32>,
    /// Update NY wet ratio
    pub update_ny_wet_ratio: Callback<f32>,
}

/// Provide the long‑lived `TunerService`.
/// Call exactly once near the application root before any usage.
pub fn provide_tuner_service() {
    // Wire backend commands (no payloads)
    let UseTauriWithReturn {
        error: reset_error,
        trigger: trigger_reset,
        ..
    } = use_command::<()>(common::commands::tuner::RESET_CONFIG);

    let UseTauriWithReturn {
        error: start_error,
        trigger: trigger_start,
        ..
    } = use_command::<()>(common::commands::tuner::START_STREAM);

    let UseTauriWithReturn {
        error: stop_error,
        trigger: trigger_stop,
        ..
    } = use_command::<()>(common::commands::tuner::STOP_STREAM);

    let UseTauriWithReturn {
        error: toggle_probe_error,
        trigger: trigger_toggle_probe,
        data: probe_active,
    } = use_command::<bool>(common::commands::tuner::TOGGLE_PROBE);

    // Get tuner config resource
    let UseTauriResourceReturn {
        data: config,
        error: config_error,
        ..
    } = use_tauri_resource::<Config>(common::commands::tuner::CONFIG);

    // Get tuner layout resource (safe-area-aware baseline, orientation, etc.)
    let UseTauriResourceReturn {
        data: layout,
        error: layout_error,
        ..
    } = use_tauri_resource::<Layout>(common::commands::tuner::LAYOUT);

    let UseTauriResourceReturn {
        data: constraints,
        error: constraints_error,
        ..
    } = use_tauri_resource::<ReflectTunerConstraints>(common::events::tuner::CONSTRAINTS);

    let UseTauriReturn {
        trigger: update_spectrum_range,
        error: update_spectrum_range_error,
        ..
    } = use_invoke::<UpdateRangePayload, (), ()>(common::commands::tuner::UPDATE_RANGE);

    let UseTauriReturn {
        trigger: update_ny_threshold,
        error: update_ny_threshold_error,
        ..
    } = use_invoke::<UpdateNyThresholdPayload, (), ()>(common::commands::tuner::UPDATE_THRESHOLD);

    let UseTauriReturn {
        trigger: update_wet_ratio,
        error: update_wet_ratio_error,
        ..
    } = use_invoke::<UpdateWetRatioPayload, (), ()>(common::commands::tuner::UPDATE_WET_RATIO);

    // Construct service with stable callbacks
    let service = TunerService {
        config: RwSignal::new(None),
        layout: RwSignal::new(None),
        spectrum: RwSignal::new(None),
        reset: Callback::new(move |_| trigger_reset(Some(()))),
        start_stream: Callback::new(move |_| trigger_start(Some(()))),
        stop_stream: Callback::new(move |_| trigger_stop(Some(()))),
        probe: Callback::new(move |_| trigger_toggle_probe(Some(()))),
        probe_active: Signal::derive(move || probe_active().unwrap_or_default()),
        constraints: Memo::new(move |_| constraints().unwrap_or_default()),
        update_spectrum_range: Callback::new(move |payload: UpdateRangePayload| {
            update_spectrum_range(Some((payload, ())));
        }),
        update_ny_threshold: Callback::new(move |ny_threshold: f32| {
            let payload = UpdateNyThresholdPayload { ny_threshold };
            update_ny_threshold(Some((payload, ())));
        }),
        update_ny_wet_ratio: Callback::new(move |wet_ratio: f32| {
            let payload = UpdateWetRatioPayload { wet_ratio };
            update_wet_ratio(Some((payload, ())));
        }),
    };

    // Centralized error logging
    Effect::new(move |_| {
        if let Some(err) = reset_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::RESET_CONFIG
            );
        }
        if let Some(err) = start_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::START_STREAM
            );
        }
        if let Some(err) = stop_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::STOP_STREAM
            );
        }
        if let Some(err) = toggle_probe_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::TOGGLE_PROBE
            );
        }

        if let Some(err) = config_error() {
            log::error!(
                "Error retrieving {}: {err}",
                common::commands::tuner::CONFIG
            );
        }

        if let Some(err) = layout_error() {
            log::error!(
                "Error retrieving {}: {err}",
                common::commands::tuner::LAYOUT
            );
        }

        if let Some(err) = constraints_error() {
            log::error!(
                "Error retrieving {}: {err}",
                common::events::tuner::CONSTRAINTS
            );
        }

        if let Some(err) = update_spectrum_range_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::UPDATE_RANGE
            );
        }

        if let Some(err) = update_ny_threshold_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::UPDATE_THRESHOLD
            );
        }

        if let Some(err) = update_wet_ratio_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::UPDATE_WET_RATIO
            );
        }
    });

    Effect::new(move |_| {
        if let Some(cfg) = config() {
            service.config.set(Some(cfg));
        }
    });

    Effect::new(move |_| {
        if let Some(lay) = layout() {
            service.layout.set(Some(lay));
        }
    });

    provide_context(service);
}

/// Retrieve the previously provided `TunerService`.
pub fn expect_tuner_service() -> TunerService {
    expect_context::<TunerService>()
}
