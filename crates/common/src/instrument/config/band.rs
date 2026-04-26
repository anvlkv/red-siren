use serde::{Deserialize, Serialize};

use super::{BandChannel, NodeConfig};

use crate::error::InstrumentConfigError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BandConfig {
    /// Output channel
    pub channel: BandChannel,
    /// Band nodes
    pub nodes: Vec<NodeConfig>,
}

impl BandConfig {
    /// Unified band validation (safe constraints only).
    pub(crate) fn validate(&self, _band_idx: usize) -> Result<(), InstrumentConfigError> {
        if self.nodes.is_empty() {
            return Err(InstrumentConfigError::EmptyBand);
        }
        for (i, n) in self.nodes.iter().enumerate() {
            n.validate(i)?;
        }
        Ok(())
    }

    /// Calculate the average distance in cents between the adjacent frequency nodes in the band.
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
