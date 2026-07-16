use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum WindowError {
    #[error("macOS window setup failed: {0}")]
    MacOSSetupError(String),
    #[error("Multiple windows are not supported")]
    MultipleWindowsNotSupported,
    #[error("Window creation failed: {0}")]
    WindowCreationError(String),
    #[error("Secondary window not permitted to invoke command: {0}")]
    SecondaryWindow(String),
    #[error("Failed to get main window")]
    NoMain,
}
