use serde::{Deserialize, Serialize};

use super::{GroupChannel, NodeConfig};

use crate::error::InstrumentConfigError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupConfig {
    /// Output channel
    pub channel: GroupChannel,
    /// Group nodes
    pub nodes: Vec<NodeConfig>,
}

impl GroupConfig {
    /// Unified group validation (safe constraints only).
    pub(crate) fn validate(&self, _group_idx: usize) -> Result<(), InstrumentConfigError> {
        if self.nodes.is_empty() {
            return Err(InstrumentConfigError::EmptyGroup);
        }
        for (i, n) in self.nodes.iter().enumerate() {
            n.validate(i)?;
        }
        Ok(())
    }
}
