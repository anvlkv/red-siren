use common::commands::health::{SetupStatePayload, SETUP_STATE};
use leptos::prelude::*;

use super::tauri_resource::{use_tauri_resource, UseTauriResourceReturn};

#[derive(Clone)]
struct SetupState(Memo<Option<SetupStatePayload>>);

pub fn provide_setup_context() {
    let UseTauriResourceReturn {
        data: setup_state, ..
    } = use_tauri_resource::<SetupStatePayload>(SETUP_STATE);

    provide_context(SetupState(Memo::new(move |_| setup_state())));
}

/// Returns whether devtools/editor features are enabled.
///
/// Derived directly from `health::SETUP_STATE`.
pub fn is_devtools_enabled() -> Memo<bool> {
    let SetupState(setup_state) = expect_context::<SetupState>();
    Memo::new(move |_| setup_state().map(|s| s.devtools).unwrap_or_default())
}

/// Returns the microphone permission state as provided by the backend:
/// - `Some(true)`  => granted
/// - `Some(false)` => denied
/// - `None`        => not requested / unknown yet
///
/// Derived directly from `health::SETUP_STATE`.
pub fn is_mic_premission_granted() -> Memo<Option<bool>> {
    let SetupState(setup_state) = expect_context::<SetupState>();
    Memo::new(move |_| setup_state().and_then(|s| s.mic_permission))
}
