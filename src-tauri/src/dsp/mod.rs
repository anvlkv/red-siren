mod analyze;
mod commands;
mod error;
mod net;
mod resonator;
mod siren;
mod snap;
mod synth;
mod t_lerp;

pub use analyze::*;
pub use commands::*;
pub use error::*;
pub use net::*;
pub use siren::*;
pub use snap::*;
pub use synth::*;

use tauri::{App, Manager};

pub fn app_setup(app: &mut App) -> Result<(), error::DspError> {
    app.manage(DspState::default());
    Ok(())
}
