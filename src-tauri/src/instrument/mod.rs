mod commands;
mod engine;

use common::{error::Result, instrument::Preset};
use tauri::{App, AppHandle, Emitter, Manager, async_runtime::spawn};
use crate::persistence::persistence::{load_json_or_default, save_json};



pub use commands::*;
pub use engine::InstrumentEngine;


pub(super) const PRESETS_STORE_NAME: &str = "presets.json";
pub(super) const PRESETS_STORE_KEY: &str = "presets";

pub fn setup(app: &mut App) -> Result<()> {
    let is_new = app.manage(engine::InstrumentEngine::new(app.handle())?);

    if is_new {
        log::debug!("Instrument engine initialized and managed state created");
        let presets: Preset = load_json_or_default(app.handle(), PRESETS_STORE_NAME, PRESETS_STORE_KEY)?;

        let windows = app.webview_windows();
        let base_handle_new = app.handle().clone();
        if let Some(window) = windows.get("main") {
            match window.inner_size() {
                Ok(size) => {
                    spawn(async move {
                        let state = base_handle_new.state::<engine::InstrumentEngine>();
                        match state.set_size(size.width as f64, size.height as f64) {
                            Ok(_) => {
                                log::debug!(
                                    "Set initial instrument layout for window size: {}x{}",
                                    size.width,
                                    size.height
                                );
                                let layout = state.layout();
                                if let Err(e) = base_handle_new
                                    .emit(common::instrument::events::LAYOUT, layout)
                                {
                                    log::error!("Failed emitting initial instrument layout: {e}");
                                }
                            }
                            Err(e) => {
                                log::error!("error setting initial instrument layout: {e}");
                            }
                        }
                        match state.set_preset(presets.clone()) {
                            Ok(_) => {
                                log::debug!("Set initial instrument presets");
                            }
                            Err(e) => {
                                log::error!("error setting initial instrument presets: {e}");
                            }
                        }
                    });
                }
                Err(e) => {
                    log::warn!("Main window size unavailable at setup (proceeding without initial layout): {e}");
                    spawn(async move {
                        let state = base_handle_new.state::<engine::InstrumentEngine>();
                        if let Err(e) = state.set_preset(presets.clone()) {
                            log::error!("error setting initial instrument presets: {e}");
                        } else {
                            log::debug!("Set initial instrument presets");
                        }
                    });
                }
            }
        } else {
            log::warn!("Main window not yet available at setup; proceeding without initial layout");
            spawn(async move {
                let state = base_handle_new.state::<engine::InstrumentEngine>();
                if let Err(e) = state.set_preset(presets.clone()) {
                    log::error!("error setting initial instrument presets: {e}");
                } else {
                    log::debug!("Set initial instrument presets");
                }
            });
        }
    } else {
        log::debug!("Instrument engine state already exists; skipping initialization");
    }

    // Window appearance updates are coordinated by AppBus; instrument listeners trimmed.

    // Window size updates are coordinated by AppBus; instrument listeners trimmed.

    Ok(())
}


pub (super) fn save_preset(preset: Preset, app: &AppHandle) -> Result<()> {
    save_json(app, PRESETS_STORE_NAME, PRESETS_STORE_KEY, &preset)?;
    log::debug!("Instrument presets saved to store");
    Ok(())
}
