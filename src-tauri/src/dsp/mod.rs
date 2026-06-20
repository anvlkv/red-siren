mod analyze;
mod commands;
mod error;
mod net;
mod synth;

pub use analyze::*;
pub use commands::*;
pub use error::*;
pub use net::*;
pub use synth::*;

use tauri::{App, Manager};

pub fn app_setup(app: &mut App) -> Result<(), error::DspError> {
    app.manage(DspState::default());
    Ok(())
}
