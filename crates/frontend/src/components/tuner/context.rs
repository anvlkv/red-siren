use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

use common::tuner::{Config, SpectrumData};

/// TunerService centralizes:
/// - Reactive tuner state shared across tuner UI layers (config, spectrum frame, active sensor selection)
/// - Stable command callbacks (reset config, start stream, stop stream)
/// - Centralized command error logging (DRY)
///
/// Rationale:
/// - Mirrored after `PlaybackService` to avoid race conditions when invoking
///   start/stop during scope teardown (page cleanup).
/// - Pages/components call these callbacks instead of wiring ephemeral `use_invoke`/`use_command`
///   instances that might be dropped too early.
/// - Keeps wiring + logging in one place (MAYA / DRY / KISS).
///
/// Provide once high in the app tree with `provide_tuner_service()`.
/// Access anywhere with `expect_tuner_service()`.
///
/// Example (in a page like `Tune`):
/// ```ignore
/// let tuner = expect_tuner_service();
/// Effect::new(move |_| tuner.start_stream.run(()));
/// on_cleanup(move || tuner.stop_stream.run(()));
/// let on_reset = move |_| tuner.reset.run(());
/// ```
#[derive(Clone)]
pub struct TunerService {
    /// Current tuner configuration (populated by listener/resource logic outside service)
    pub config: RwSignal<Option<Config>>,
    /// Latest spectrum snapshot
    pub spectrum: RwSignal<Option<SpectrumData>>,
    /// Currently selected sensor index (UI state)
    pub active_sensor: RwSignal<Option<usize>>,

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

    // Construct service with stable callbacks
    let service = TunerService {
        config: RwSignal::new(None),
        spectrum: RwSignal::new(None),
        active_sensor: RwSignal::new(None),
        reset: Callback::new(move |_| trigger_reset(Some(()))),
        start_stream: Callback::new(move |_| trigger_start(Some(()))),
        stop_stream: Callback::new(move |_| trigger_stop(Some(()))),
        probe: Callback::new(move |_| trigger_toggle_probe(Some(()))),
        probe_active: Signal::derive(move || probe_active().unwrap_or_default()),
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
    });

    provide_context(service);
}

/// Retrieve the previously provided `TunerService`.
pub fn expect_tuner_service() -> TunerService {
    expect_context::<TunerService>()
}
