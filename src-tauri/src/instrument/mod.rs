mod commands;
mod engine;

use shared::error::Result;
use tauri::{async_runtime::spawn, App, Listener, Manager};

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
            state.set_is_dark(is_dark).await;
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
            state.set_size(window_state.width, window_state.height).await;
        });
    });

    Ok(())
}
