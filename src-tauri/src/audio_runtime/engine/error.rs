use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum AudioEngineError {
    #[error("Input stream not supported by selected input device")]
    InputNotSupportedBySelectedInputDevice,
    #[error("Output stream not supported by selected output device")]
    OutputNotSupportedBySelectedOutputDevice,
    #[error("No output device available")]
    NoOutputDeviceAvailable,
    #[error("No input device available")]
    NoInputDeviceAvailable,
    #[error("Device unavailable")]
    DeviceUnavailable,
}
