use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum InstrumentError {
    /// Instrument excitement channel was closed
    #[error("excite channel closed")]
    ExciteChannelClosed,
}
