use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AudioStreamError {
    #[error("failed to start audio stream: {0}")]
    Startup(String),
    #[error("sample format not supported: {0}")]
    UnsupportedSampleFormat(String),
    #[error("build stream failed: {0}")]
    BuildStream(String),
}
