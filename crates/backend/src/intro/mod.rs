mod commands;
mod engine;

use common::error::Result;
use tauri::{App, Manager};

pub use commands::*;

pub fn setup(app: &mut App) -> Result<()> {
    app.manage(engine::IntroEngineState::new());

    Ok(())
}
