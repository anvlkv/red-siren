use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum AudioRuntimeError {
    #[error("audio stream error: {0}")]
    StreamError(#[from] super::stream::AudioStreamError),
    #[error("audio engine error: {0}")]
    EngineError(#[from] super::engine::AudioEngineError),
    #[error("audio runtime not yet initialized")]
    NotInitialized,
}
