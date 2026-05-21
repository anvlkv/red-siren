use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AudioAnalysisError {
    /// A generic error for spectrum analyzer failures.
    #[error("spectrum analyzer error: {0}")]
    SpectrumAnalyzerError(String),
}
