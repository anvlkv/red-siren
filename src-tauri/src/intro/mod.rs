mod commands;
mod engine;

use tauri::{App, Manager};

pub use commands::*;

pub fn setup(app: &mut App) -> Result<(), String> {
    app.manage(engine::IntroEngineState::new());

    Ok(())
}
