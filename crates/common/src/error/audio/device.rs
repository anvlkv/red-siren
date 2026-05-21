use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AudioDeviceError {
    #[error("audio device unavailable")]
    Unavailable,
    #[error("host unavailable")]
    HostUnavailable,
    #[error("device ID not found")]
    InvalidDeviceId,
    #[error("device does not support input")]
    NoSupportForInput,
    #[error("device does not support output")]
    NoSupportForOutput,
}
