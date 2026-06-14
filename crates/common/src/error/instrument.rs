use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::NodeKey;

#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum InstrumentError {
    /// Instrument excitement channel was full
    #[error("excite channel full")]
    ExciteChannelFull,

    /// Instrument excitement channel was closed
    #[error("excite channel closed")]
    ExciteChannelClosed,

    /// Node key not found
    #[error("unknown node key: {0:?}")]
    UnknownNodeKey(NodeKey),

    /// Other error with a message
    #[error("unknown error: {0}")]
    Other(String),
}
