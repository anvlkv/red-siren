mod commands;
mod engine;

use shared::error::Result;
use tauri::{App, Manager};

pub use commands::*;

pub fn setup(app: &mut App) -> Result<()> {
    app.manage(engine::InstrumentEngine::default());

    Ok(())
}
