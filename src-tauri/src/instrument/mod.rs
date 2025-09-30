mod commands;
mod engine;

use shared::error::Result;
use tauri::{async_runtime::spawn, App, Emitter, Listener, Manager};

pub use commands::*;

use crate::setup::WindowState;

pub fn setup(app: &mut App) -> Result<()> {
    app.manage(engine::InstrumentEngine::default());


    let base_handle_appearance = app.handle().clone();
    app.listen(shared::events::setup::UPDATE_WINDOW_APPEARANCE, move |_| {
        let handle = base_handle_appearance.clone();
        // Acquire state objects inside spawned task so they have 'static lifetime relative to task.
        spawn(async move {
            let state = handle.state::<engine::InstrumentEngine>();
            let win_state = handle.state::<WindowState>();
            let is_dark = win_state.lock().await.dark;
            if let Err(e) = state.set_is_dark(is_dark).await {
                log::error!("error updating `{}`: {e}", shared::events::setup::UPDATE_WINDOW_APPEARANCE)
            }
        });
    });

    let base_handle_size = app.handle().clone();
    app.listen(shared::events::setup::UPDATE_WINDOW_SIZE, move |_| {
        let handle = base_handle_size.clone();
        // Acquire state objects inside spawned task so they have 'static lifetime relative to task.
        spawn(async move {
            let state = handle.state::<engine::InstrumentEngine>();
            let win_state = handle.state::<WindowState>();
            let window_state = win_state.lock().await;
            match state.set_size(window_state.width, window_state.height).await {
                Ok(_) => {
                    let layout = state.inner.layout.lock().await;
                    if let Err(e) = handle.emit(shared::instrument::events::LAYOUT, *layout) {
                        log::error!("Failed emitting instrument layout: {e}");
                    }
                }
                Err(e) => {
                    log::error!("error updating `{}`: {e}", shared::events::setup::UPDATE_WINDOW_SIZE)
                }
            }
        });
    });

    Ok(())
}
