mod audio_session;
mod commands;
mod engine;
mod error;
mod stream;

use tauri::{App, Manager};

pub use commands::*;
pub use engine::{AudioEngine, DeviceData, PlaybackState};
pub use error::*;
pub use stream::SampleType;

pub fn app_setup(app: &mut App) -> Result<(), error::AudioRuntimeError> {
    app.manage(engine::AudioEngine::new());
    Ok(())
}
