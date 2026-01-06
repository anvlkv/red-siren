mod commands;
mod engine;

use common::{error::{AppError, InstrumentError, Result, SetupError}, instrument::{Preset, commands::{ReflectBandControlPayload, ReflectKeyControlPayload}, events::{BAND_CONTROL_G_K, KEY_CONTROL_G_K}}};
use tauri::{App, AppHandle, Emitter, Manager, async_runtime::spawn};
use tauri_plugin_store::StoreExt;

use crate::setup::WindowState;

pub use commands::*;
pub use engine::InstrumentEngine;


pub(super) const PRESETS_STORE_NAME: &str = "presets.json";
pub(super) const PRESETS_STORE_KEY: &str = "presets";

pub fn setup(app: &mut App) -> Result<()> {
    let is_new = app.manage(engine::InstrumentEngine::new(app.handle())?);

    if is_new {
        log::debug!("Instrument engine initialized and managed state created");
        let store = app.store(PRESETS_STORE_NAME).map_err(|e| {
            AppError::Setup(SetupError::StoreSetupErr {
                message: e.to_string(),
            })
        })?;
        let presets = store.get(PRESETS_STORE_KEY).and_then(|val| serde_json::from_value::<Preset>(val).ok()).unwrap_or_default();

        let windows = app.webview_windows();
        let window = windows.get("main").ok_or(AppError::Setup(SetupError::MainWindowMissing))?;
        let size = window.inner_size()?;
        let base_handle_new = app.handle().clone();
        spawn(async move {
            let state = base_handle_new.state::<engine::InstrumentEngine>();
            match state.set_size(size.width as f64, size.height as f64) {
                Ok(_) => {
                    log::debug!("Set initial instrument layout for window size: {}x{}", size.width, size.height);
                    let layout = state.layout();
                    // Emit initial layouts and config
                    if let Err(e) = base_handle_new.emit(common::instrument::events::LAYOUT, layout) {
                        log::error!("Failed emitting initial instrument layout: {e}");
                    }
                }
                Err(e) => {
                    log::error!("error setting initial instrument layout: {e}");
                }
            }
            match state.set_preset(presets) {
                Ok(_) => {
                    log::debug!("Set initial instrument presets");
                },
                Err(e) => {
                    log::error!("error setting initial instrument presets: {e}");
                },
            }
        });
    } else {
        log::debug!("Instrument engine state already exists; skipping initialization");
    }

    // Window appearance updates are coordinated by AppBus; instrument listeners trimmed.

    // Window size updates are coordinated by AppBus; instrument listeners trimmed.

    Ok(())
}


pub (super) fn save_preset(preset: Preset, app: &AppHandle) -> Result<()> {
    let store = app.store(PRESETS_STORE_NAME).map_err(|e| {
        AppError::Instrument(InstrumentError::PresetStoreError(e.to_string(),
        ))
    })?;
    let preset_value = serde_json::to_value(&preset)
        .map_err(|e| AppError::Instrument(InstrumentError::PresetSerializationError(e.to_string())))?;
    store.set(PRESETS_STORE_KEY, preset_value);
    store.save().map_err(|e| {
        AppError::Instrument(InstrumentError::PresetStoreError(e.to_string(),
        ))
    })?;
    log::debug!("Instrument presets saved to store");
    Ok(())
}
