mod commands;
mod engine;

use common::error::{AppError, Result, SetupError};
use tauri::{async_runtime::spawn, App, Emitter, Listener, Manager};

use crate::setup::WindowState;

pub use commands::*;
pub use engine::InstrumentEngine;


pub fn setup(app: &mut App) -> Result<()> {
    let is_new = app.manage(engine::InstrumentEngine::new(app.handle())?);

    if is_new {
        log::debug!("Instrument engine initialized and managed state created");
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
        });
    } else {
        log::debug!("Instrument engine state already exists; skipping initialization");
    }

    let base_handle_appearance = app.handle().clone();
    app.listen(common::events::setup::UPDATE_WINDOW_APPEARANCE, move |_| {
        let handle = base_handle_appearance.clone();
        log::debug!("Received UPDATE_WINDOW_APPEARANCE event");
        // Acquire state objects inside spawned task so they have 'static lifetime relative to task.
        spawn(async move {
            let state = handle.state::<engine::InstrumentEngine>();
            let win_state = handle.state::<WindowState>();
            let is_dark = win_state.lock().dark;
            if let Err(e) = state.set_is_dark(is_dark) {
                log::error!("error updating `{}`: {e}", common::events::setup::UPDATE_WINDOW_APPEARANCE)
            }

            if let Err(e) = handle.emit(common::instrument::events::LAYOUT, state.layout()) {
                log::error!("Failed emitting instrument layout: {e}");
            }
        });
    });

    let base_handle_size = app.handle().clone();
    app.listen(common::events::setup::UPDATE_WINDOW_SIZE, move |_| {
        let handle = base_handle_size.clone();
        // Acquire state objects inside spawned task so they have 'static lifetime relative to task.
        log::debug!("Received UPDATE_WINDOW_SIZE event");
        spawn(async move {
            log::trace!("updating engine state");
            let state = handle.state::<engine::InstrumentEngine>();
            let win_state = handle.state::<WindowState>();
            log::trace!("acquired states, locking window state");
            let window_state = win_state.lock();
            log::trace!("locked window state: {:#?}", *window_state);
            log::trace!("Setting instrument layout for new window size: {}x{}", window_state.width, window_state.height);
            match state.set_size(window_state.width, window_state.height) {
                Ok(_) => {
                    log::debug!("Updated instrument layout for new window size: {}x{}", window_state.width, window_state.height);
                    if let Err(e) = handle.emit(common::instrument::events::LAYOUT, state.layout()) {
                        log::error!("Failed emitting instrument layout: {e}");
                    }
                }
                Err(e) => {
                    log::error!("error updating `{}`: {e}", common::events::setup::UPDATE_WINDOW_SIZE)
                }
            }
        });
    });

    Ok(())
}
