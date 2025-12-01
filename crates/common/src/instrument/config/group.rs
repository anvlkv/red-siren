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

    /// Calculate the average distance in cents between the adjacent frequency nodes in the group.
    pub fn distance_cents(&self) -> f64 {
        self.nodes
            .chunks(2)
            .rev()
            .map(|d| {
                if d.len() == 2 {
                    let c1 = d[0].cents;
                    let c2 = d[1].cents;
                    c2 - c1
                } else {
                    0.0
                }
            })
            .sum::<f64>()
            / (self.nodes.len() - 1) as f64
    }
}
