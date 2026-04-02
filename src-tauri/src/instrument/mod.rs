mod commands;
mod state;

use crate::persistence::persistence::{load_json_or_default, save_json};
use common::{device::DeviceData, error::Result, instrument::Preset};
use tauri::{async_runtime::spawn, App, AppHandle, Emitter, Manager};

pub use commands::*;
pub use state::InstrumentState;

pub(super) const INSTRUMENT_STORE_NAME: &str = "instrument.json";
pub(super) const PRESETS_STORE_KEY: &str = "presets";
pub(super) const INPUT_DEVICE_STORE_KEY: &str = "input_device";
pub(super) const OUTPUT_DEVICE_STORE_KEY: &str = "output_device";

pub fn setup(app: &mut App) -> Result<()> {
    let is_new = app.manage(state::InstrumentState::new(app.handle())?);

    if is_new {
        log::debug!("Instrument state initialized and managed state created");
        let presets: Preset =
            load_json_or_default(app.handle(), INSTRUMENT_STORE_NAME, PRESETS_STORE_KEY)?;

        let windows = app.webview_windows();
        let base_handle_new = app.handle().clone();
        if let Some(window) = windows.get("main") {
            match window.inner_size() {
                Ok(size) => {
                    spawn(async move {
                        let state = base_handle_new.state::<state::InstrumentState>();
                        match state.set_size(size.width as f64, size.height as f64) {
                            Ok(_) => {
                                log::debug!(
                                    "Set initial instrument layout for window size: {}x{}",
                                    size.width,
                                    size.height
                                );
                                let layout = state.layout();
                                if let Err(e) =
                                    base_handle_new.emit(common::instrument::events::LAYOUT, layout)
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
                        let state = base_handle_new.state::<state::InstrumentState>();
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
                let state = base_handle_new.state::<state::InstrumentState>();
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

    Ok(())
}

pub(super) fn save_preset(preset: Preset, app: &AppHandle) -> Result<()> {
    save_json(app, INSTRUMENT_STORE_NAME, PRESETS_STORE_KEY, &preset)?;
    log::debug!("Instrument presets saved to store");
    Ok(())
}

pub(super) fn save_input_device(device: DeviceData, app: &AppHandle) -> Result<()> {
    save_json(
        app,
        INSTRUMENT_STORE_NAME,
        INPUT_DEVICE_STORE_KEY,
        &Some(device),
    )?;
    log::debug!("Input device saved to store");
    Ok(())
}

pub(super) fn save_output_device(device: DeviceData, app: &AppHandle) -> Result<()> {
    save_json(
        app,
        INSTRUMENT_STORE_NAME,
        OUTPUT_DEVICE_STORE_KEY,
        &Some(device),
    )?;
    log::debug!("Output device saved to store");
    Ok(())
}

pub(super) fn load_input_device(app: &AppHandle) -> Result<Option<DeviceData>> {
    load_json_or_default::<Option<DeviceData>>(app, INSTRUMENT_STORE_NAME, INPUT_DEVICE_STORE_KEY)
}

pub(super) fn load_output_device(app: &AppHandle) -> Result<Option<DeviceData>> {
    load_json_or_default::<Option<DeviceData>>(app, INSTRUMENT_STORE_NAME, OUTPUT_DEVICE_STORE_KEY)
}

pub(super) fn load_preset(app: &AppHandle) -> Result<Preset> {
    load_json_or_default::<Preset>(app, INSTRUMENT_STORE_NAME, PRESETS_STORE_KEY)
}
