use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

/// PlaybackService centralizes long-lived playback command triggers and error logging.
/// Why: Calling `use_command` triggers from a page's cleanup can race with scope teardown,
/// dropping the invoke before it reaches the backend. This service lives at the app root,
/// so pages can safely call `start/stop` in mount/cleanup without races.
///
/// Pattern:
/// - Provide once at app root via `provide_playback_service()`.
/// - Access anywhere with `expect_playback_service()`.
/// - Use `service.start.run(())` to start and `service.stop.run(())` to stop.
/// - Errors are logged here, keeping pages DRY and simple.
#[derive(Clone)]
pub struct PlaybackService {
    /// Start playback (invoke `common::instrument::commands::PLAYBACK_START`)
    pub start: Callback<()>,
    /// Pause playback (invoke `common::instrument::commands::PLAYBACK_PAUSE`)
    pub pause: Callback<()>,
    /// Resume playback (invoke `common::instrument::commands::PLAYBACK_RESUME`)
    pub resume: Callback<()>,
    /// Stop playback (invoke `common::instrument::commands::PLAYBACK_STOP`)
    pub stop: Callback<()>,
}

/// Provide the long-lived `PlaybackService` at the app root.
/// Call this once in your top-level `App()` before rendering routes/components.
pub fn provide_playback_service() {
    // Wire all commands once, here, so they outlive individual pages.
    let UseTauriWithReturn {
        error: start_error,
        trigger: trigger_start,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_START);

    let UseTauriWithReturn {
        error: pause_error,
        trigger: trigger_pause,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_PAUSE);

    let UseTauriWithReturn {
        error: resume_error,
        trigger: trigger_resume,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_RESUME);

    let UseTauriWithReturn {
        error: stop_error,
        trigger: trigger_stop,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_STOP);

    // Expose stable callbacks so callers can simply run `Callback::run(())`
    // Note: keeping the `Some(())` payload consistent with tauri_use::use_command pattern.
    let service = PlaybackService {
        start: Callback::new(move |_| trigger_start(Some(()))),
        pause: Callback::new(move |_| trigger_pause(Some(()))),
        resume: Callback::new(move |_| trigger_resume(Some(()))),
        stop: Callback::new(move |_| trigger_stop(Some(()))),
    };

    // Centralized error logging to keep pages DRY and KISS.
    Effect::new(move |_| {
        if let Some(err) = start_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_START
            );
        }
        if let Some(err) = pause_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_PAUSE
            );
        }
        if let Some(err) = resume_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_RESUME
            );
        }
        if let Some(err) = stop_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_STOP
            );
        }
    });

    provide_context(service);
}

/// Retrieve the previously provided `PlaybackService`.
/// Panics if the service hasn't been provided at the app root.
pub fn expect_playback_service() -> PlaybackService {
    expect_context::<PlaybackService>()
}
