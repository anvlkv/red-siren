use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum DspError {
    #[error("Synthesizer must be initialized")]
    SynthNotInitialized,
    #[error("Chamber with index {0} not present among {1} chambers")]
    NoChamberWithIndex(usize, usize),
    #[error("Tauri error: {0}")]
    TauriError(String),
}
